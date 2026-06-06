//! Tauri-managed application state.
//!
//! After the brew-domain sweep this holds only the agency subsystems:
//! the corpus cache, persisted settings (the source of truth for the
//! network/feature gates), the updater mirror, and the resolved
//! app-data directory that the corpus / install / github / updater
//! modules derive their paths from.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

use crate::commands::settings::{self, SettingsLoadState};
use crate::commands::updater::UpdaterState;
use crate::error::BrewError;

/// Shared application state. Registered via `Builder::manage()`.
pub struct AppState {
    /// Resolved app-data root — the OS-canonical
    /// `~/Library/Application Support/brew-browser/` directory. The
    /// corpus, install ledger, github cache, and settings file all
    /// derive their paths from this; the security gates that check "is
    /// this path inside our app data dir?" anchor on it too.
    pub app_data_dir: PathBuf,

    /// Phase 1 (corpus) — memoized in-memory corpus (parsed agents +
    /// index). Built lazily on the first `corpus_*` command (seed + parse
    /// + persist index), then served from this cache. `corpus_refresh`
    /// swaps the inner Arc after re-indexing the freshly-fetched tree.
    /// Mirrors the `categories_cache` lazy-`Option<Arc<_>>` pattern.
    pub corpus_cache: Arc<Mutex<Option<Arc<crate::corpus::Corpus>>>>,

    /// Single-flight mutex for `corpus_refresh`, same contract as
    /// `catalog_refresh_in_flight`.
    pub corpus_refresh_in_flight: Arc<Mutex<()>>,

    /// Persisted user settings (Phase 12d). Three-state container that
    /// distinguishes file-absent (defaults apply) from file-corrupt
    /// (fail closed — every outbound call denied until repaired).
    /// `require_network` consults this on the first line of every
    /// network-touching command.
    pub settings: Arc<RwLock<SettingsLoadState>>,

    /// Phase 15 — in-memory mirror of the latest update check + cached
    /// `Available` payload. The auto-check scheduler updates this on
    /// every wake, and `update_install` validates the caller-supplied
    /// version arg against the cached entry to defend against UI
    /// staleness. See `crate::commands::updater::UpdaterState` for the
    /// shape and the rationale.
    pub updater_state: Arc<RwLock<UpdaterState>>,
}

impl AppState {
    /// Build the state at startup. Resolves the app-data directory and
    /// loads persisted settings; the corpus and updater caches start
    /// empty and hydrate lazily on first use.
    pub fn build() -> Result<Self, BrewError> {
        let app_data_dir = resolve_app_data_dir()?;
        if !app_data_dir.exists() {
            std::fs::create_dir_all(&app_data_dir).map_err(|e| BrewError::Io {
                message: format!(
                    "could not create app data dir {}: {}",
                    app_data_dir.display(),
                    e
                ),
            })?;
        }

        // Load settings synchronously at startup. The loader handles
        // file-absent (FirstLaunch → defaults), file-corrupt (Corrupt →
        // fail closed in `require_network`), and good parse (Loaded(s)).
        // Tracing warnings for corrupt cases happen inside the loader.
        let settings_state = settings::load_at_startup(&app_data_dir);
        if matches!(settings_state, SettingsLoadState::Corrupt { .. }) {
            tracing::warn!(
                "settings: load failed at startup; require_network will deny outbound calls until user resets"
            );
        }

        Ok(Self {
            app_data_dir,
            corpus_cache: Arc::new(Mutex::new(None)),
            corpus_refresh_in_flight: Arc::new(Mutex::new(())),
            settings: Arc::new(RwLock::new(settings_state)),
            updater_state: crate::commands::updater::empty_state(),
        })
    }

    /// Consult paranoid mode + settings load state. Returns `Ok(())` if
    /// the outbound call is allowed, or `BrewError::ParanoidModeBlocked`
    /// otherwise. **Every outbound command must call this as its first
    /// line** — see the security review §12d "Cross-cutting concerns".
    ///
    /// Three cases:
    /// - `Loaded(s)` with `paranoid_mode == false` → allow.
    /// - `FirstLaunch` → allow (defaults apply, paranoid OFF — preserves
    ///   the v0.1.0 behaviour for users with no settings file yet).
    /// - `Loaded(s)` with `paranoid_mode == true` OR `Corrupt(...)` →
    ///   deny. Corrupt is a deliberate fail-closed: we don't know what
    ///   the user wanted, so we don't make outbound calls until they
    ///   repair the file (or hit Reset to defaults in the UI).
    pub async fn require_network(&self, feature: &'static str) -> Result<(), BrewError> {
        let guard = self.settings.read().await;
        match &*guard {
            SettingsLoadState::Loaded(s) if !s.paranoid_mode => Ok(()),
            SettingsLoadState::FirstLaunch => Ok(()),
            SettingsLoadState::Loaded(_) | SettingsLoadState::Corrupt { .. } => {
                Err(BrewError::ParanoidModeBlocked {
                    feature: feature.to_string(),
                })
            }
        }
    }

    /// v0.4.0 — composed gate for endpoints that are *both* network-
    /// gated by paranoid mode AND opt-in via a per-feature toggle. Used
    /// by [`crate::commands::trending::trending_history_fetch`] and
    /// future features hitting `brew-browser.zerologic.com/*`.
    ///
    /// Returns:
    /// - `Ok(())` if paranoid is OFF **and** `enhanced_trending_enabled`
    ///   is `true`.
    /// - `Err(ParanoidModeBlocked { feature })` if paranoid would deny
    ///   `require_network` (master switch wins; the per-feature toggle
    ///   is irrelevant when paranoid is on).
    /// - `Err(FeatureDisabled { feature })` if paranoid allows but the
    ///   per-feature toggle is off (or `FirstLaunch` — fresh-install
    ///   posture is opt-in only).
    ///
    /// Fail-closed on `Corrupt` is handled by the inner `require_network`
    /// call — `Corrupt` always denies first with `ParanoidModeBlocked`.
    pub async fn require_enhanced_trending(&self) -> Result<(), BrewError> {
        // Master paranoid gate first — same error variant other endpoints
        // use, so the frontend toast routing stays uniform.
        self.require_network("trending_history").await?;
        // Per-feature opt-in gate. FirstLaunch defaults are `false` for
        // this field by design (see Settings::default in commands/settings.rs).
        let guard = self.settings.read().await;
        match &*guard {
            SettingsLoadState::Loaded(s) if s.enhanced_trending_enabled => Ok(()),
            _ => Err(BrewError::FeatureDisabled {
                feature: "trending_history".to_string(),
            }),
        }
    }

    /// Composed gate for the opt-in live enrichment surface (fresh categories +
    /// descriptions fetched from `brew-browser.zerologic.com/enrichment/*`).
    /// Mirrors [`Self::require_enhanced_trending`]: master paranoid switch
    /// first, then the per-feature `live_enrichment_enabled` toggle. Used by
    /// the `enrichment_live_*` commands before any network call.
    ///
    /// Fail-closed on `Corrupt` is handled by the inner `require_network` call.
    pub async fn require_live_enrichment(&self) -> Result<(), BrewError> {
        self.require_network("live_enrichment").await?;
        let guard = self.settings.read().await;
        match &*guard {
            SettingsLoadState::Loaded(s) if s.live_enrichment_enabled => Ok(()),
            _ => Err(BrewError::FeatureDisabled {
                feature: "live_enrichment".to_string(),
            }),
        }
    }

    /// v0.5.0 — composed gate for the vulnerability-scanning surface
    /// (`brew vulns` subprocess + OSV roundtrip + optional GHSA enrich).
    /// Composes the master paranoid switch with the per-feature
    /// `vulnerability_scanning_enabled` toggle. Used by
    /// [`crate::commands::vulns::*`] before any subprocess spawn or
    /// network call.
    ///
    /// Returns:
    /// - `Ok(())` if paranoid is OFF **and** `vulnerability_scanning_enabled`
    ///   is `true`.
    /// - `Err(ParanoidModeBlocked { feature })` if paranoid would deny
    ///   `require_network` (master switch wins; the per-feature toggle
    ///   is irrelevant when paranoid is on).
    /// - `Err(FeatureDisabled { feature })` if paranoid allows but the
    ///   per-feature toggle is off (or `FirstLaunch` — fresh-install
    ///   posture is opt-in only).
    ///
    /// Fail-closed on `Corrupt` is handled by the inner `require_network`
    /// call — `Corrupt` always denies first with `ParanoidModeBlocked`.
    pub async fn require_vulnerability_scanning(&self) -> Result<(), BrewError> {
        self.require_network("vulnerability_scanning").await?;
        let guard = self.settings.read().await;
        match &*guard {
            SettingsLoadState::Loaded(s) if s.vulnerability_scanning_enabled => Ok(()),
            _ => Err(BrewError::FeatureDisabled {
                feature: "vulnerability_scanning".to_string(),
            }),
        }
    }
}

/// Resolve the canonical app-data root:
/// `~/Library/Application Support/brew-browser/`. The corpus, install
/// ledger, github cache, and settings file all derive their paths from
/// this; the security gates that check "is this path inside our app data
/// dir?" anchor on it too.
fn resolve_app_data_dir() -> Result<PathBuf, BrewError> {
    let mut base = dirs::data_dir().ok_or_else(|| BrewError::Internal {
        message: "could not resolve OS data dir".into(),
    })?;
    base.push("brew-browser");
    Ok(base)
}

/// Tauri setup hook — instantiates and manages `AppState`.
pub fn initialize<R: tauri::Runtime>(
    app: &mut tauri::App<R>,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri::Manager;
    let state = AppState::build()?;
    app.manage(state);
    Ok(())
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::settings::Settings;

    /// Build a minimal AppState whose only meaningful field is `settings`.
    /// All other fields use whatever `AppState::build` resolves — for the
    /// gate-only tests below the brew-path lookup, catalog load, etc., are
    /// irrelevant. Settings slot is overwritten *after* construction so we
    /// don't depend on whatever happens to be on disk for the test user.
    async fn build_state_with(slot: SettingsLoadState) -> AppState {
        let state = AppState::build().expect("AppState::build");
        {
            let mut guard = state.settings.write().await;
            *guard = slot;
        }
        state
    }

    #[tokio::test]
    async fn require_network_allows_first_launch() {
        let state = build_state_with(SettingsLoadState::FirstLaunch).await;
        assert!(state.require_network("trending_fetch").await.is_ok());
    }

    #[tokio::test]
    async fn require_network_allows_loaded_with_paranoid_off() {
        let s = Settings {
            paranoid_mode: false,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        assert!(state.require_network("catalog_refresh").await.is_ok());
    }

    #[tokio::test]
    async fn require_network_blocks_when_paranoid_on() {
        let s = Settings {
            paranoid_mode: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        let r = state.require_network("trending_fetch").await;
        match r {
            Err(BrewError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "trending_fetch");
            }
            other => panic!("expected ParanoidModeBlocked, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn require_network_blocks_when_corrupt() {
        // Fail-closed: corrupt settings file → deny even though paranoid
        // would default false. This is the load-bearing security gate from
        // the §12d review.
        let state = build_state_with(SettingsLoadState::Corrupt {
            message: "bad json".into(),
        })
        .await;
        let r = state.require_network("cask_icon_from_homepage").await;
        match r {
            Err(BrewError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "cask_icon_from_homepage");
            }
            other => panic!("expected ParanoidModeBlocked from corrupt, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn require_network_feature_string_round_trips() {
        // The static-str argument must be carried verbatim into the error
        // so the frontend can route the toast to the right setting.
        let state = build_state_with(SettingsLoadState::Corrupt {
            message: "x".into(),
        })
        .await;
        for feat in ["trending_fetch", "cask_icon_from_homepage", "catalog_refresh"] {
            let r = state.require_network(feat).await;
            match r {
                Err(BrewError::ParanoidModeBlocked { feature }) => {
                    assert_eq!(feature, feat);
                }
                other => panic!("expected block for {feat}, got {other:?}"),
            }
        }
    }

    // ---------- v0.4.0: require_enhanced_trending ----------

    #[tokio::test]
    async fn require_enhanced_trending_allows_when_toggle_on_and_paranoid_off() {
        let s = Settings {
            paranoid_mode: false,
            enhanced_trending_enabled: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        assert!(state.require_enhanced_trending().await.is_ok());
    }

    #[tokio::test]
    async fn require_enhanced_trending_blocks_when_toggle_off() {
        // Toggle off → FeatureDisabled (NOT ParanoidModeBlocked — the
        // cure is a different setting toggle).
        let s = Settings {
            paranoid_mode: false,
            enhanced_trending_enabled: false,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        match state.require_enhanced_trending().await {
            Err(BrewError::FeatureDisabled { feature }) => {
                assert_eq!(feature, "trending_history");
            }
            other => panic!("expected FeatureDisabled, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn require_enhanced_trending_blocks_when_paranoid_on_even_if_toggle_on() {
        // Master switch wins. Frontend should route to Offline Mode
        // toggle, not the per-feature toggle.
        let s = Settings {
            paranoid_mode: true,
            enhanced_trending_enabled: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        match state.require_enhanced_trending().await {
            Err(BrewError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "trending_history");
            }
            other => panic!("expected ParanoidModeBlocked, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn require_enhanced_trending_blocks_on_first_launch() {
        // FirstLaunch → require_network allows but the per-feature
        // toggle defaults to false → FeatureDisabled. Critical for
        // first-install posture: zero zerologic.com traffic until opt-in.
        let state = build_state_with(SettingsLoadState::FirstLaunch).await;
        match state.require_enhanced_trending().await {
            Err(BrewError::FeatureDisabled { feature }) => {
                assert_eq!(feature, "trending_history");
            }
            other => panic!("expected FeatureDisabled, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn require_enhanced_trending_blocks_when_corrupt() {
        // Corrupt → paranoid gate fires first with ParanoidModeBlocked.
        // Important: this is the same error code other endpoints emit
        // on Corrupt, so the toast UX stays uniform across features.
        let state = build_state_with(SettingsLoadState::Corrupt {
            message: "boom".into(),
        })
        .await;
        match state.require_enhanced_trending().await {
            Err(BrewError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "trending_history");
            }
            other => panic!("expected ParanoidModeBlocked from corrupt, got {other:?}"),
        }
    }

    // ---------- v0.5.0: require_vulnerability_scanning ----------

    #[tokio::test]
    async fn require_vulnerability_scanning_allows_when_toggle_on_and_paranoid_off() {
        let s = Settings {
            paranoid_mode: false,
            vulnerability_scanning_enabled: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        assert!(state.require_vulnerability_scanning().await.is_ok());
    }

    #[tokio::test]
    async fn require_vulnerability_scanning_blocks_when_toggle_off() {
        // Toggle off → FeatureDisabled (NOT ParanoidModeBlocked — the
        // cure is a different setting toggle).
        let s = Settings {
            paranoid_mode: false,
            vulnerability_scanning_enabled: false,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        match state.require_vulnerability_scanning().await {
            Err(BrewError::FeatureDisabled { feature }) => {
                assert_eq!(feature, "vulnerability_scanning");
            }
            other => panic!("expected FeatureDisabled, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn require_vulnerability_scanning_blocks_when_paranoid_on_even_if_toggle_on() {
        // Master switch wins. Frontend should route to Offline Mode
        // toggle, not the per-feature toggle.
        let s = Settings {
            paranoid_mode: true,
            vulnerability_scanning_enabled: true,
            ..Settings::default()
        };
        let state = build_state_with(SettingsLoadState::Loaded(s)).await;
        match state.require_vulnerability_scanning().await {
            Err(BrewError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "vulnerability_scanning");
            }
            other => panic!("expected ParanoidModeBlocked, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn require_vulnerability_scanning_blocks_on_first_launch() {
        // FirstLaunch → require_network allows but the per-feature
        // toggle defaults to false → FeatureDisabled. Critical for
        // first-install posture: zero `brew vulns` invocations and zero
        // OSV traffic until opt-in.
        let state = build_state_with(SettingsLoadState::FirstLaunch).await;
        match state.require_vulnerability_scanning().await {
            Err(BrewError::FeatureDisabled { feature }) => {
                assert_eq!(feature, "vulnerability_scanning");
            }
            other => panic!("expected FeatureDisabled, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn require_vulnerability_scanning_blocks_when_corrupt() {
        // Corrupt → paranoid gate fires first with ParanoidModeBlocked,
        // same as every other composed gate. Keeps the toast routing
        // uniform across features.
        let state = build_state_with(SettingsLoadState::Corrupt {
            message: "boom".into(),
        })
        .await;
        match state.require_vulnerability_scanning().await {
            Err(BrewError::ParanoidModeBlocked { feature }) => {
                assert_eq!(feature, "vulnerability_scanning");
            }
            other => panic!("expected ParanoidModeBlocked from corrupt, got {other:?}"),
        }
    }
}
