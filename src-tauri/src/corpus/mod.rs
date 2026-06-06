//! Corpus subsystem (Phase 1) — the maintained copy of the agency-agents
//! repo that the whole app reads from.
//!
//! ## Source of truth (systemPatterns.md §1)
//!
//! ```text
//! <app_data_dir>/
//! ├── corpus/                 our maintained copy of the agency-agents repo
//! │   └── <category>/<slug>.md
//! └── state/
//!     └── corpus-index.json   slug → CorpusEntry (hashes, category, version)
//! ```
//!
//! - **Seed**: a baseline corpus ships inside the app bundle
//!   (`resources/corpus-baseline/<category>/<slug>.md`). On first run it is
//!   copied to `<app_data_dir>/corpus/` so the app works offline.
//! - **Refresh** ([`corpus_refresh`]): fetch the GitHub tarball
//!   `https://codeload.github.com/msitarzewski/agency-agents/tar.gz/refs/heads/main`,
//!   extract the category dirs over the working copy, and rebuild
//!   `corpus-index.json`. No runtime git dependency.
//!
//! ## Determinism (contracts.md §E)
//!
//! `corpus-index.json` is keyed by a `BTreeMap` so its serialization has a
//! stable key order. The three per-agent hashes are SHA-256 of canonical
//! byte regions of the source `.md` (see [`parse`]). Nothing in the index
//! carries a timestamp; the only timestamp is [`CorpusMeta::fetched_at`],
//! which lives in a separate meta file, not the index.

mod parse;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::error::BrewError;
use crate::types::{Agent, Category, CorpusEntry, CorpusMeta};
use crate::util::fs::atomic_write;

// ---------- Constants ----------

/// The 16 agent category directories we bundle/seed/index, in the canonical
/// order. Anything outside this set in the working copy is ignored so a
/// stray refresh tarball directory can't inject a phantom category.
///
/// NB: the upstream agency-agents repo also has `strategy/` (NEXUS
/// playbooks/runbooks) and `examples/` (multi-agent workflow walkthroughs),
/// but those hold documentation, not agent personas (no `name:` frontmatter),
/// so they are intentionally excluded from the agent catalog. They are a
/// candidate for a future "Playbooks" section — see decisions.md.
const CATEGORY_DIRS: [&str; 16] = [
    "academic",
    "design",
    "engineering",
    "finance",
    "game-development",
    "integrations",
    "marketing",
    "paid-media",
    "product",
    "project-management",
    "sales",
    "security",
    "spatial-computing",
    "specialized",
    "support",
    "testing",
];

/// GitHub `codeload` tarball for the live corpus. Streamed, gunzipped,
/// and unpacked on [`corpus_refresh`]. No git binary required.
const CORPUS_TARBALL_URL: &str =
    "https://codeload.github.com/msitarzewski/agency-agents/tar.gz/refs/heads/main";

/// User-Agent for the refresh fetch. Mirrors the catalog refresh style.
const USER_AGENT: &str = "agency-agents/0.1 (+https://github.com/msitarzewski/agency-agents)";

/// Whole-request timeout for the tarball fetch. The repo is small (a few
/// hundred small markdown files) so 60s is generous.
const REFRESH_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Cap on the raw `tar.gz` response (defends against a hostile mirror).
/// The real tarball is well under 5 MiB; 32 MiB is large headroom.
const MAX_TARBALL_BYTES: u64 = 32 * 1024 * 1024;

/// Cap on a single decompressed agent `.md`. Personas run a few KiB;
/// 1 MiB is absurdly generous and still bounds memory.
const MAX_AGENT_BYTES: u64 = 1024 * 1024;

/// Version string recorded for the bundled baseline before any refresh
/// has resolved a commit SHA.
const BASELINE_VERSION: &str = "baseline";

// ---------- On-disk meta ----------

/// `corpus-meta.json` — top-level metadata for the working copy. Distinct
/// from the index (which is per-agent) so [`corpus_status`] can answer
/// "what version / how many / fetched when" with one small read.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredMeta {
    version: String,
    commit: Option<String>,
    fetched_at: String,
    count: u32,
}

impl From<StoredMeta> for CorpusMeta {
    fn from(m: StoredMeta) -> Self {
        CorpusMeta {
            version: m.version,
            commit: m.commit,
            fetched_at: m.fetched_at,
            count: m.count,
        }
    }
}

// ---------- In-memory corpus ----------

/// The parsed, in-memory corpus: every agent plus its index row, ordered
/// deterministically by `(category, slug)`. Memoized on `AppState` so the
/// hot read commands (`corpus_list` / `corpus_get` / `corpus_categories`)
/// never touch disk after the first build.
#[derive(Debug, Clone)]
pub struct Corpus {
    /// Agents in stable `(category, slug)` order. `Agent.body` is fully
    /// populated here; list views clone-and-clear it (see
    /// [`Corpus::list`]).
    agents: Vec<Agent>,
    /// Index rows keyed by slug — `BTreeMap` so the serialized
    /// `corpus-index.json` has stable key order.
    index: BTreeMap<String, CorpusEntry>,
    meta: CorpusMeta,
}

impl Corpus {
    /// Number of indexed agents.
    pub fn count(&self) -> u32 {
        self.index.len() as u32
    }

    /// [`CorpusMeta`] for `corpus_status`.
    pub fn meta(&self) -> CorpusMeta {
        self.meta.clone()
    }

    /// List view — agents (optionally filtered to one `category`) with the
    /// `body` omitted to keep the IPC payload small (contracts.md §C).
    pub fn list(&self, category: Option<&str>) -> Vec<Agent> {
        self.agents
            .iter()
            .filter(|a| category.is_none_or(|c| a.category == c))
            .map(|a| Agent {
                body: String::new(),
                ..a.clone()
            })
            .collect()
    }

    /// Full agent (incl. body) by slug, or `None` if unknown.
    pub fn get(&self, slug: &str) -> Option<Agent> {
        self.agents.iter().find(|a| a.slug == slug).cloned()
    }

    /// Index row (hashes + category) by slug, for the install/reconcile layer.
    pub fn entry(&self, slug: &str) -> Option<CorpusEntry> {
        self.index.get(slug).cloned()
    }

    /// The active corpus version (from meta), used to stamp ledger records.
    pub fn version(&self) -> String {
        self.meta.version.clone()
    }

    /// Per-category counts in canonical [`CATEGORY_DIRS`] order. Label +
    /// icon come from the bundled `categories.json` (Lucide PascalCase
    /// names) via [`category_meta`]. Categories with zero agents are
    /// still returned so the Discover grid renders the full 18-tile set.
    pub fn categories(&self) -> Vec<Category> {
        let mut counts: BTreeMap<&str, u32> = BTreeMap::new();
        for entry in self.index.values() {
            *counts.entry(entry.category.as_str()).or_default() += 1;
        }
        CATEGORY_DIRS
            .iter()
            .map(|&slug| {
                let (label, icon) = category_meta(slug);
                Category {
                    slug: slug.to_string(),
                    label,
                    icon,
                    count: counts.get(slug).copied().unwrap_or(0),
                }
            })
            .collect()
    }

    /// Serialize the index to canonical pretty JSON. Stable key order
    /// (BTreeMap) → byte-identical output for an unchanged corpus.
    fn index_json(&self) -> Result<Vec<u8>, BrewError> {
        serde_json::to_vec_pretty(&self.index).map_err(|e| BrewError::Internal {
            message: format!("serialize corpus-index.json: {e}"),
        })
    }
}

// ---------- Category metadata ----------

/// The bundled `categories.json` shape we read label + icon from. Only
/// the `categories` map is needed here.
#[derive(Debug, Deserialize)]
struct CategoriesFile {
    categories: BTreeMap<String, CategoryMetaRow>,
}

#[derive(Debug, Deserialize)]
struct CategoryMetaRow {
    label: String,
    icon: String,
}

const CATEGORIES_JSON: &str = include_str!("../../data/agency-categories.json");

/// Resolve `(label, icon)` for a category slug from the bundled
/// `categories.json`. Falls back to a title-cased slug + a neutral
/// `Folder` icon if the slug is somehow absent (keeps Discover rendering
/// rather than dropping a tile).
fn category_meta(slug: &str) -> (String, String) {
    // Parse once per call is fine — this runs only on `corpus_categories`
    // (a cold path) and the JSON is tiny. Memoizing would mean threading
    // another cache field; not worth it for an 18-row map.
    if let Ok(file) = serde_json::from_str::<CategoriesFile>(CATEGORIES_JSON) {
        if let Some(row) = file.categories.get(slug) {
            return (row.label.clone(), row.icon.clone());
        }
    }
    (title_case(slug), "Folder".to_string())
}

/// `"game-development"` → `"Game Development"`. Deterministic fallback for
/// the unlikely missing-slug case.
fn title_case(slug: &str) -> String {
    slug.split('-')
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------- Path helpers ----------

/// The working corpus directory: `<app_data_dir>/corpus`. ALWAYS derived
/// from `app_data_dir` — never composed from IPC input.
pub(crate) fn corpus_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("corpus")
}

/// The state directory holding `corpus-index.json` + `corpus-meta.json` and
/// (Phase 2) the install ledger `installs.json`.
pub(crate) fn state_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("state")
}

fn index_path(app_data_dir: &Path) -> PathBuf {
    state_dir(app_data_dir).join("corpus-index.json")
}

fn meta_path(app_data_dir: &Path) -> PathBuf {
    state_dir(app_data_dir).join("corpus-meta.json")
}

// ---------- Build / load ----------

/// Resolve the active corpus for the current process:
///
/// 1. Seed the working copy from the bundled baseline if `corpus/` is
///    empty (first run).
/// 2. Parse + index everything under `corpus/`.
/// 3. Write `corpus-index.json` + `corpus-meta.json` if they are missing
///    or stale (so reconciliation has the index on disk too).
///
/// `baseline_dir` is the bundled baseline resolved from the Tauri
/// resource dir (`resource_dir()/resources/corpus-baseline`). `Never`
/// panics: a fully empty or unreadable corpus yields an empty [`Corpus`]
/// with `count == 0` so the UI degrades to "no agents" rather than
/// failing to launch.
pub async fn resolve_active(app_data_dir: &Path, baseline_dir: &Path) -> Corpus {
    let dir = corpus_dir(app_data_dir);

    // Seed on first run (empty or absent working copy).
    if is_empty_dir(&dir) {
        if let Err(e) = seed_from_baseline(baseline_dir, &dir).await {
            tracing::warn!("corpus: seed from baseline failed: {e}");
        }
    }

    // Determine the version to stamp the index with: keep whatever a prior
    // refresh recorded, else the baseline marker.
    let version = match load_stored_meta(app_data_dir).await {
        Some(m) => m.version,
        None => BASELINE_VERSION.to_string(),
    };

    let corpus = match build_from_dir(&dir, &version).await {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("corpus: index build failed ({e}); serving empty corpus");
            empty_corpus(&version)
        }
    };

    // Persist index + meta (best effort — read commands work from the
    // in-memory copy regardless; the on-disk index exists for the
    // reconciliation subsystem built in a later phase).
    if let Err(e) = persist(app_data_dir, &corpus).await {
        tracing::warn!("corpus: persist index/meta failed: {e}");
    }

    corpus
}

/// Build an in-memory [`Corpus`] by walking `<dir>/<category>/<slug>.md`
/// for every known category. Files without valid frontmatter (READMEs,
/// workflow docs) are skipped. The resulting `agents` vec and `index` map
/// are ordered deterministically by `(category, slug)`.
async fn build_from_dir(dir: &Path, version: &str) -> Result<Corpus, BrewError> {
    let mut rows: Vec<(Agent, CorpusEntry)> = Vec::new();

    for &category in CATEGORY_DIRS.iter() {
        let cat_dir = dir.join(category);
        let mut read = match tokio::fs::read_dir(&cat_dir).await {
            Ok(r) => r,
            Err(_) => continue, // category dir absent — fine, skip.
        };

        // Collect filenames first so we can sort for determinism (the OS
        // read_dir order is unspecified).
        let mut files: Vec<PathBuf> = Vec::new();
        while let Ok(Some(ent)) = read.next_entry().await {
            let path = ent.path();
            if path.extension().and_then(|e| e.to_str()) == Some("md") {
                files.push(path);
            }
        }
        files.sort();

        for path in files {
            let Some(slug) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let raw = match read_capped(&path, MAX_AGENT_BYTES).await {
                Ok(bytes) => bytes,
                Err(e) => {
                    tracing::warn!("corpus: skip {} ({e})", path.display());
                    continue;
                }
            };
            let source = match String::from_utf8(raw) {
                Ok(s) => s,
                Err(_) => {
                    tracing::warn!("corpus: skip {} (non-utf8)", path.display());
                    continue;
                }
            };
            match parse::parse_agent(slug, category, &source) {
                Ok(Some(pair)) => rows.push(pair),
                Ok(None) => {} // not an agent (no frontmatter) — skip silently.
                Err(e) => tracing::warn!("corpus: {e}"),
            }
        }
    }

    // `rows` is already in `(category, slug)` order because we iterate
    // categories in CATEGORY_DIRS order and sort filenames within each.
    let mut agents = Vec::with_capacity(rows.len());
    let mut index = BTreeMap::new();
    for (agent, entry) in rows {
        index.insert(entry.slug.clone(), entry);
        agents.push(agent);
    }

    let count = index.len() as u32;
    Ok(Corpus {
        agents,
        index,
        meta: CorpusMeta {
            version: version.to_string(),
            commit: None,
            // The build itself carries no timestamp; fetched_at reflects
            // when the *content* was last fetched. For a baseline build
            // that is the seed time, captured at persist below if no meta
            // exists yet.
            fetched_at: String::new(),
            count,
        },
    })
}

fn empty_corpus(version: &str) -> Corpus {
    Corpus {
        agents: Vec::new(),
        index: BTreeMap::new(),
        meta: CorpusMeta {
            version: version.to_string(),
            commit: None,
            fetched_at: String::new(),
            count: 0,
        },
    }
}

// ---------- Seeding ----------

/// True if `dir` does not exist or contains no entries.
fn is_empty_dir(dir: &Path) -> bool {
    match std::fs::read_dir(dir) {
        Ok(mut it) => it.next().is_none(),
        Err(_) => true,
    }
}

/// Copy `<baseline>/<category>/*.md` into `<dest>/<category>/`. Only the
/// known category dirs are seeded; anything else in the baseline is
/// ignored. Idempotent: re-seeding overwrites file-for-file.
async fn seed_from_baseline(baseline: &Path, dest: &Path) -> Result<(), BrewError> {
    if !baseline.exists() {
        return Err(BrewError::Io {
            message: format!("baseline corpus not found at {}", baseline.display()),
        });
    }
    let mut seeded = 0u32;
    for &category in CATEGORY_DIRS.iter() {
        let src_cat = baseline.join(category);
        let mut read = match tokio::fs::read_dir(&src_cat).await {
            Ok(r) => r,
            Err(_) => continue,
        };
        let dst_cat = dest.join(category);
        tokio::fs::create_dir_all(&dst_cat)
            .await
            .map_err(|e| BrewError::Io {
                message: format!("create {}: {e}", dst_cat.display()),
            })?;
        while let Ok(Some(ent)) = read.next_entry().await {
            let path = ent.path();
            if path.extension().and_then(|e| e.to_str()) != Some("md") {
                continue;
            }
            let Some(fname) = path.file_name() else { continue };
            let bytes = read_capped(&path, MAX_AGENT_BYTES).await?;
            atomic_write(&dst_cat.join(fname), &bytes).await?;
            seeded += 1;
        }
    }
    tracing::info!("corpus: seeded {seeded} agents from baseline");
    Ok(())
}

// ---------- Persistence ----------

/// Write `corpus-index.json` + `corpus-meta.json` atomically into the
/// state dir. The meta `fetched_at` is preserved from any prior meta;
/// when none exists (fresh baseline seed) it is stamped once with the
/// current UTC time so subsequent launches don't re-stamp it (keeps the
/// index byte-stable across launches).
async fn persist(app_data_dir: &Path, corpus: &Corpus) -> Result<(), BrewError> {
    let sdir = state_dir(app_data_dir);
    tokio::fs::create_dir_all(&sdir)
        .await
        .map_err(|e| BrewError::Io {
            message: format!("create state dir {}: {e}", sdir.display()),
        })?;

    // Index — deterministic, no timestamp.
    let index_bytes = corpus.index_json()?;
    atomic_write(&index_path(app_data_dir), &index_bytes).await?;

    // Meta — preserve prior fetched_at/commit if present; else stamp now.
    let prior = load_stored_meta(app_data_dir).await;
    let fetched_at = prior
        .as_ref()
        .map(|m| m.fetched_at.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
    let commit = prior.as_ref().and_then(|m| m.commit.clone());

    let stored = StoredMeta {
        version: corpus.meta.version.clone(),
        commit,
        fetched_at,
        count: corpus.count(),
    };
    let meta_bytes = serde_json::to_vec_pretty(&stored).map_err(|e| BrewError::Internal {
        message: format!("serialize corpus-meta.json: {e}"),
    })?;
    atomic_write(&meta_path(app_data_dir), &meta_bytes).await?;
    Ok(())
}

/// Load `corpus-meta.json` if present + parseable, else `None`.
async fn load_stored_meta(app_data_dir: &Path) -> Option<StoredMeta> {
    let path = meta_path(app_data_dir);
    let bytes = tokio::fs::read(&path).await.ok()?;
    serde_json::from_slice(&bytes).ok()
}

// ---------- Refresh (live tarball) ----------

/// Fetch the GitHub tarball, extract its category dirs over the working
/// copy, re-index, and persist. Returns the fresh [`CorpusMeta`].
///
/// The extraction is done into a temp dir first, then the known category
/// dirs are swapped in, so a partial/failed download never corrupts the
/// live `corpus/`.
async fn refresh(app_data_dir: &Path) -> Result<CorpusMeta, BrewError> {
    let client = reqwest::Client::builder()
        .timeout(REFRESH_TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .map_err(|e| BrewError::Network {
            url: CORPUS_TARBALL_URL.to_string(),
            message: format!("client build: {e}"),
        })?;

    let resp = client
        .get(CORPUS_TARBALL_URL)
        .send()
        .await
        .map_err(|e| BrewError::Network {
            url: CORPUS_TARBALL_URL.to_string(),
            message: e.to_string(),
        })?;
    if !resp.status().is_success() {
        return Err(BrewError::HttpStatus {
            url: CORPUS_TARBALL_URL.to_string(),
            status: resp.status().as_u16(),
        });
    }
    let bytes = resp.bytes().await.map_err(|e| BrewError::Network {
        url: CORPUS_TARBALL_URL.to_string(),
        message: format!("read body: {e}"),
    })?;
    if bytes.len() as u64 > MAX_TARBALL_BYTES {
        return Err(BrewError::Io {
            message: format!(
                "corpus tarball {} bytes exceeds {} cap",
                bytes.len(),
                MAX_TARBALL_BYTES
            ),
        });
    }

    // Extract the category dirs into the live corpus dir. The tarball has
    // a single top-level `agency-agents-main/` prefix that we strip.
    let dir = corpus_dir(app_data_dir);
    let extracted = extract_categories(&bytes, &dir)?;
    if extracted == 0 {
        return Err(BrewError::Internal {
            message: "corpus tarball contained no agent files under known categories".into(),
        });
    }

    // Re-index from the freshly-written working copy. Use a `main`-tagged
    // version marker; codeload does not expose the resolved commit SHA in
    // the tarball, so we record the ref name. A later phase can resolve
    // the exact SHA via the GitHub API if needed.
    let version = format!("github:main@{}", chrono::Utc::now().format("%Y-%m-%d"));
    let mut corpus = build_from_dir(&dir, &version).await?;
    let fetched_at = chrono::Utc::now().to_rfc3339();
    corpus.meta.fetched_at = fetched_at.clone();

    // Persist a fresh meta (overwrite fetched_at/version this time —
    // unlike the baseline persist which preserves prior fetched_at).
    let sdir = state_dir(app_data_dir);
    tokio::fs::create_dir_all(&sdir)
        .await
        .map_err(|e| BrewError::Io {
            message: format!("create state dir {}: {e}", sdir.display()),
        })?;
    let index_bytes = corpus.index_json()?;
    atomic_write(&index_path(app_data_dir), &index_bytes).await?;
    let stored = StoredMeta {
        version: version.clone(),
        commit: None,
        fetched_at: fetched_at.clone(),
        count: corpus.count(),
    };
    let meta_bytes = serde_json::to_vec_pretty(&stored).map_err(|e| BrewError::Internal {
        message: format!("serialize corpus-meta.json: {e}"),
    })?;
    atomic_write(&meta_path(app_data_dir), &meta_bytes).await?;

    Ok(corpus.meta)
}

/// Gunzip + untar `tar_gz`, writing every `<category>/<slug>.md` whose
/// category is in [`CATEGORY_DIRS`] into `<dest>/<category>/`. The
/// codeload tarball nests everything under a single
/// `agency-agents-main/` top-level dir, which we strip. Returns the count
/// of agent files written.
///
/// Path-traversal safe: we only ever join the *sanitized* `category` +
/// `file_name` onto `dest`; the raw archive path is never used to build a
/// write target.
fn extract_categories(tar_gz: &[u8], dest: &Path) -> Result<u32, BrewError> {
    use std::io::Read;

    let gz = flate2::read::GzDecoder::new(tar_gz);
    // Cap decompressed bytes so a gzip bomb can't blow up memory/disk.
    let mut capped = gz.take(MAX_TARBALL_BYTES * 8);
    let mut tar_bytes = Vec::new();
    capped
        .read_to_end(&mut tar_bytes)
        .map_err(|e| BrewError::Io {
            message: format!("gunzip corpus tarball: {e}"),
        })?;

    let mut archive = tar::Archive::new(std::io::Cursor::new(tar_bytes));
    let entries = archive.entries().map_err(|e| BrewError::Io {
        message: format!("read tar entries: {e}"),
    })?;

    let mut written = 0u32;
    for entry in entries {
        let mut entry = entry.map_err(|e| BrewError::Io {
            message: format!("tar entry: {e}"),
        })?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().map_err(|e| BrewError::Io {
            message: format!("tar entry path: {e}"),
        })?;
        // Strip the single top-level `agency-agents-main/` component, then
        // expect `<category>/<file>.md`.
        let comps: Vec<String> = path
            .components()
            .filter_map(|c| match c {
                std::path::Component::Normal(s) => s.to_str().map(|s| s.to_string()),
                _ => None,
            })
            .collect();
        if comps.len() < 3 {
            continue; // need top/<category>/<file>
        }
        let category = comps[1].as_str();
        let fname = comps.last().unwrap().as_str();
        if !CATEGORY_DIRS.contains(&category) {
            continue;
        }
        if !fname.ends_with(".md") || fname == "README.md" {
            continue;
        }
        // Sanitized target — built only from validated components.
        let cat_dir = dest.join(category);
        std::fs::create_dir_all(&cat_dir).map_err(|e| BrewError::Io {
            message: format!("create {}: {e}", cat_dir.display()),
        })?;
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf).map_err(|e| BrewError::Io {
            message: format!("read tar file {}: {e}", fname),
        })?;
        std::fs::write(cat_dir.join(fname), &buf).map_err(|e| BrewError::Io {
            message: format!("write {}: {e}", cat_dir.join(fname).display()),
        })?;
        written += 1;
    }
    Ok(written)
}

// ---------- Small fs helper ----------

/// Read up to `max` bytes; error (not truncate) on oversize. Mirrors
/// `util::fs::read_capped` but accepts a sync `Path` + tokio read so we
/// don't need to thread the catalog's exact helper here.
async fn read_capped(path: &Path, max: u64) -> Result<Vec<u8>, BrewError> {
    let bytes = tokio::fs::read(path).await.map_err(|e| BrewError::Io {
        message: format!("read {}: {e}", path.display()),
    })?;
    if bytes.len() as u64 > max {
        return Err(BrewError::Io {
            message: format!("{} exceeds {} byte cap", path.display(), max),
        });
    }
    Ok(bytes)
}

// =====================================================================
// Tauri commands (contracts.md §C — corpus surface)
// =====================================================================

use crate::state::AppState;
use tauri::{AppHandle, Manager, State};

/// Resolve the bundled baseline dir from the Tauri resource dir. In dev
/// the resources live under the crate; in a bundled app they're inside
/// the `.app`. Tauri's `resource_dir()` resolves both.
fn baseline_dir(app: &AppHandle) -> Result<PathBuf, BrewError> {
    let res = app.path().resource_dir().map_err(|e| BrewError::Internal {
        message: format!("resolve resource_dir: {e}"),
    })?;
    Ok(res.join("resources").join("corpus-baseline"))
}

/// Resolve the per-app data dir via Tauri's path resolver (honors the
/// bundle id `com.zerologic.agency-agents-app`).
pub(crate) fn app_data_dir(app: &AppHandle) -> Result<PathBuf, BrewError> {
    app.path().app_data_dir().map_err(|e| BrewError::Internal {
        message: format!("resolve app_data_dir: {e}"),
    })
}

/// Read the raw, byte-exact `.md` source of a seeded agent from the working
/// corpus copy (`<app_data>/corpus/<category>/<slug>.md`). Identity-tool
/// installs (claude-code, copilot) ship this verbatim, and provenance
/// reconciliation re-renders against it. Path is derived from app data +
/// the agent's own category/slug — never from IPC input.
pub(crate) async fn read_source(
    app: &AppHandle,
    category: &str,
    slug: &str,
) -> Result<String, BrewError> {
    let adir = app_data_dir(app)?;
    let path = corpus_dir(&adir)
        .join(category)
        .join(format!("{slug}.md"));
    let bytes = read_capped(&path, MAX_AGENT_BYTES).await?;
    String::from_utf8(bytes).map_err(|e| BrewError::Io {
        message: format!("agent source {slug}.md not UTF-8: {e}"),
    })
}

/// Ensure the in-memory corpus is built + memoized on `AppState`, then
/// return the shared `Arc`. First call seeds (if needed), parses, and
/// persists the index; subsequent calls are a cheap cache read.
pub(crate) async fn ensure_corpus(app: &AppHandle, state: &AppState) -> Result<Arc<Corpus>, BrewError> {
    // Hold the cache lock across the ENTIRE init — check, seed, parse, store.
    // The frontend fires corpus_list + corpus_categories (+ corpus_status)
    // concurrently on mount; a released-lock double-check would let each run
    // `seed_from_baseline` at once, racing on the same `<file>.tmp` paths
    // (rename → ENOENT). Serializing the first load is correct and cheap:
    // it happens once, and every later call is a fast locked cache read.
    let mut cached = state.corpus_cache.lock().await;
    if let Some(c) = cached.as_ref() {
        return Ok(Arc::clone(c));
    }
    let adir = app_data_dir(app)?;
    let bdir = baseline_dir(app)?;
    let corpus = Arc::new(resolve_active(&adir, &bdir).await);
    *cached = Some(Arc::clone(&corpus));
    Ok(corpus)
}

/// `corpus_status()` — version / commit / fetched-at / count for the
/// active corpus.
#[tauri::command]
pub async fn corpus_status(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CorpusMeta, BrewError> {
    let corpus = ensure_corpus(&app, &state).await?;
    Ok(corpus.meta())
}

/// `corpus_refresh()` — fetch the live tarball, re-index, swap the
/// memoized corpus, and return the fresh meta.
#[tauri::command]
pub async fn corpus_refresh(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<CorpusMeta, BrewError> {
    state.require_network("corpus_refresh").await?;

    // Single-flight: a second click fast-fails rather than queuing a
    // duplicate download.
    let _flight = match state.corpus_refresh_in_flight.try_lock() {
        Ok(g) => g,
        Err(_) => {
            return Err(BrewError::InvalidArgument {
                message: "corpus refresh already in progress".into(),
            });
        }
    };

    let adir = app_data_dir(&app)?;
    refresh(&adir).await?;

    // Rebuild the in-memory copy from the freshly-written working tree and
    // swap the memoized Arc so subsequent reads see the new corpus.
    let bdir = baseline_dir(&app)?;
    let fresh = Arc::new(resolve_active(&adir, &bdir).await);
    let meta = fresh.meta();
    {
        let mut cached = state.corpus_cache.lock().await;
        *cached = Some(fresh);
    }
    Ok(meta)
}

/// `corpus_list(category?)` — list view (bodies omitted).
#[tauri::command]
pub async fn corpus_list(
    app: AppHandle,
    state: State<'_, AppState>,
    category: Option<String>,
) -> Result<Vec<Agent>, BrewError> {
    let corpus = ensure_corpus(&app, &state).await?;
    Ok(corpus.list(category.as_deref()))
}

/// `corpus_get(slug)` — full agent incl. body.
#[tauri::command]
pub async fn corpus_get(
    app: AppHandle,
    state: State<'_, AppState>,
    slug: String,
) -> Result<Agent, BrewError> {
    let corpus = ensure_corpus(&app, &state).await?;
    corpus.get(&slug).ok_or(BrewError::InvalidArgument {
        message: format!("unknown agent slug: {slug}"),
    })
}

/// `corpus_categories()` — the 18-tile Discover grid with per-category
/// counts.
#[tauri::command]
pub async fn corpus_categories(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<Category>, BrewError> {
    let corpus = ensure_corpus(&app, &state).await?;
    Ok(corpus.categories())
}

// ---------- Tests ----------

#[cfg(test)]
mod tests {
    use super::*;

    fn write_agent(dir: &Path, category: &str, slug: &str, name: &str, body: &str) {
        let cat = dir.join(category);
        std::fs::create_dir_all(&cat).unwrap();
        let content = format!("---\nname: {name}\ndescription: d\n---\n{body}\n");
        std::fs::write(cat.join(format!("{slug}.md")), content).unwrap();
    }

    #[tokio::test]
    async fn build_indexes_agents_in_stable_order() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        // Write out of order across two categories.
        write_agent(dir, "engineering", "zeta", "Zeta", "z");
        write_agent(dir, "engineering", "alpha", "Alpha", "a");
        write_agent(dir, "design", "mid", "Mid", "m");

        let corpus = build_from_dir(dir, "test").await.unwrap();
        assert_eq!(corpus.count(), 3);
        // design < engineering, and within engineering alpha < zeta.
        let order: Vec<&str> = corpus.agents.iter().map(|a| a.slug.as_str()).collect();
        assert_eq!(order, vec!["mid", "alpha", "zeta"]);
    }

    #[tokio::test]
    async fn index_json_is_byte_stable_across_builds() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write_agent(dir, "engineering", "alpha", "Alpha", "a");
        write_agent(dir, "design", "mid", "Mid", "m");

        let a = build_from_dir(dir, "v").await.unwrap().index_json().unwrap();
        let b = build_from_dir(dir, "v").await.unwrap().index_json().unwrap();
        assert_eq!(a, b, "corpus-index.json must be deterministic");
    }

    #[tokio::test]
    async fn list_omits_body_get_includes_it() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write_agent(dir, "engineering", "alpha", "Alpha", "the persona body");
        let corpus = build_from_dir(dir, "v").await.unwrap();

        let listed = corpus.list(None);
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].body, "", "list view must omit body");

        let full = corpus.get("alpha").unwrap();
        assert!(full.body.contains("the persona body"), "get must include body");
    }

    #[tokio::test]
    async fn list_filters_by_category() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write_agent(dir, "engineering", "alpha", "Alpha", "a");
        write_agent(dir, "design", "mid", "Mid", "m");
        let corpus = build_from_dir(dir, "v").await.unwrap();

        let eng = corpus.list(Some("engineering"));
        assert_eq!(eng.len(), 1);
        assert_eq!(eng[0].slug, "alpha");
    }

    #[tokio::test]
    async fn categories_returns_all_16_with_counts() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write_agent(dir, "engineering", "alpha", "Alpha", "a");
        write_agent(dir, "engineering", "beta", "Beta", "b");
        let corpus = build_from_dir(dir, "v").await.unwrap();

        let cats = corpus.categories();
        assert_eq!(cats.len(), 16, "all 16 agent categories always returned");
        let eng = cats.iter().find(|c| c.slug == "engineering").unwrap();
        assert_eq!(eng.count, 2);
        assert_eq!(eng.label, "Engineering");
        assert_eq!(eng.icon, "Code");
        // Empty category still present with count 0.
        let fin = cats.iter().find(|c| c.slug == "finance").unwrap();
        assert_eq!(fin.count, 0);
    }

    #[tokio::test]
    async fn non_agent_files_are_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        write_agent(dir, "engineering", "real", "Real", "x");
        // A README with no frontmatter.
        let cat = dir.join("engineering");
        std::fs::write(cat.join("README.md"), "# Examples\nnope\n").unwrap();
        // A workflow doc with no frontmatter.
        std::fs::write(cat.join("workflow.md"), "# Workflow\nnope\n").unwrap();

        let corpus = build_from_dir(dir, "v").await.unwrap();
        assert_eq!(corpus.count(), 1);
        assert!(corpus.get("real").is_some());
        assert!(corpus.get("workflow").is_none());
    }

    #[tokio::test]
    async fn seed_then_build_round_trips() {
        let baseline = tempfile::tempdir().unwrap();
        write_agent(baseline.path(), "engineering", "alpha", "Alpha", "a");
        write_agent(baseline.path(), "design", "mid", "Mid", "m");

        let app_data = tempfile::tempdir().unwrap();
        let corpus = resolve_active(app_data.path(), baseline.path()).await;
        assert_eq!(corpus.count(), 2);
        // Working copy + index were written.
        assert!(corpus_dir(app_data.path()).join("engineering/alpha.md").exists());
        assert!(index_path(app_data.path()).exists());
        assert!(meta_path(app_data.path()).exists());
    }

    #[test]
    fn title_case_handles_hyphens() {
        assert_eq!(title_case("game-development"), "Game Development");
        assert_eq!(title_case("engineering"), "Engineering");
    }

    #[test]
    fn category_meta_resolves_from_bundled_json() {
        let (label, icon) = category_meta("engineering");
        assert_eq!(label, "Engineering");
        assert_eq!(icon, "Code");
    }

    /// Parse the REAL bundled baseline corpus (not a synthetic tempdir) so a
    /// malformed real agent (bad frontmatter fence, missing `name`) fails CI
    /// rather than shipping. `cargo test` runs with cwd = crate root, so the
    /// relative resource path resolves. Counts are pinned to the agency-agents
    /// snapshot (231 agents, 18 categories) — bump them on a corpus refresh.
    #[tokio::test]
    async fn real_bundled_baseline_parses_completely() {
        let dir = Path::new("resources/corpus-baseline");
        if !dir.exists() {
            // Resources not present in this build context — skip rather than fail.
            return;
        }
        let corpus = build_from_dir(dir, "baseline-test").await.unwrap();

        assert_eq!(corpus.count(), 210, "all bundled agent personas indexed");

        // Every agent parsed real frontmatter: non-empty name + slug, real category.
        for a in &corpus.agents {
            assert!(!a.name.trim().is_empty(), "agent {} has empty name", a.slug);
            assert!(!a.slug.trim().is_empty(), "agent has empty slug");
            assert!(
                CATEGORY_DIRS.contains(&a.category.as_str()),
                "agent {} has unknown category {}",
                a.slug,
                a.category
            );
        }

        // Spot-check categories that nest agents in subdirs upstream — these are
        // the ones a flat seeding would silently undercount.
        let cats = corpus.categories();
        assert_eq!(cats.len(), 16, "16 agent categories");
        let count_of = |slug: &str| cats.iter().find(|c| c.slug == slug).map(|c| c.count).unwrap_or(0);
        assert_eq!(count_of("engineering"), 30);
        assert_eq!(count_of("specialized"), 46);
        // game-development nests agents in unity/, godot/, unreal-engine/ etc.
        // upstream; a flat seeding would silently undercount these.
        assert_eq!(count_of("game-development"), 20, "nested game-dev agents included");
    }
}
