//! Tool registry — the single source of truth for supported tools.
//!
//! Loaded from the embedded `data/tools/*.json` directory (whole-directory
//! embed, no per-file index): **adding a tool is adding a JSON file.** Mirrors
//! the frontend `toolRegistry.ts`, which globs the same files. The backend reads
//! tool metadata (label, detect dirs, version command, dest path templates,
//! render `format`, scope) from here rather than hardcoding it per tool.

use std::sync::OnceLock;

use include_dir::{include_dir, Dir};
use serde::Deserialize;

static TOOLS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/data/tools");

/// Scope capabilities — whether a tool can deploy user-globally and/or per-project.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ScopeCaps {
    #[serde(default)]
    pub user: bool,
    #[serde(default)]
    pub project: bool,
}

/// Detection hints: dirs whose presence implies the tool is installed.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Detect {
    #[serde(default)]
    pub dirs: Vec<String>,
    #[serde(default)]
    pub agents_dir: Option<String>,
}

/// `<bin> <args…>` probe for the installed version.
#[derive(Debug, Clone, Deserialize)]
pub struct VersionCmd {
    pub bin: String,
    #[serde(default)]
    pub args: Vec<String>,
}

/// Destination path templates (relative; `{slug}` substituted). User paths are
/// rooted at `$HOME`, project paths at the project root.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct Dest {
    #[serde(default)]
    pub user: Vec<String>,
    #[serde(default)]
    pub project: Vec<String>,
}

/// One tool's full definition, as authored in `data/tools/<kebab>.json`.
///
/// Some fields (`short`, `accent`, `icon`) exist to mirror the frontend
/// `toolRegistry.ts` and to validate that every bundled JSON parses cleanly;
/// the Rust backend doesn't consume them yet, hence `dead_code` is allowed.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct ToolMeta {
    /// camelCase id — the wire value used by the frontend + install ledger.
    pub id: String,
    /// Full display name, e.g. "Claude Code".
    pub label: String,
    #[serde(default)]
    pub short: String,
    /// kebab id matching the CLI install scripts, e.g. "claude-code".
    pub kebab: String,
    #[serde(default)]
    pub accent: String,
    #[serde(default)]
    pub icon: Option<String>,
    /// Installable today (has a native renderer) vs merely recognized.
    #[serde(default)]
    pub wired: bool,
    #[serde(default)]
    pub order: Option<u32>,
    #[serde(default)]
    pub scope: Option<ScopeCaps>,
    #[serde(default)]
    pub detect: Option<Detect>,
    #[serde(default)]
    pub version: Option<VersionCmd>,
    /// Renderer key (e.g. "identity", "codex-toml"); None ⇒ not installable.
    #[serde(default)]
    pub format: Option<String>,
    /// "source" (keep the corpus filename) or "name" (slugify frontmatter name).
    #[serde(default)]
    pub slug_from: Option<String>,
    /// Namespace prepended to the output slug AND the rendered `name` field —
    /// e.g. "agency-" for skill-dir tools that share a global skills folder.
    #[serde(default)]
    pub slug_prefix: Option<String>,
    #[serde(default)]
    pub dest: Option<Dest>,
}

impl ToolMeta {
    pub fn supports_user(&self) -> bool {
        self.scope.as_ref().is_some_and(|s| s.user)
    }
    pub fn supports_project(&self) -> bool {
        self.scope.as_ref().is_some_and(|s| s.project)
    }
}

/// Parse + cache the registry on first access. Panics on malformed JSON — that's
/// a build-time authoring error in a bundled file, not a runtime condition.
fn registry() -> &'static Vec<ToolMeta> {
    static REG: OnceLock<Vec<ToolMeta>> = OnceLock::new();
    REG.get_or_init(|| {
        let mut v: Vec<ToolMeta> = TOOLS_DIR
            .files()
            .filter(|f| f.path().extension().is_some_and(|e| e == "json"))
            .map(|f| {
                serde_json::from_slice(f.contents())
                    .unwrap_or_else(|e| panic!("invalid tool registry file {:?}: {e}", f.path()))
            })
            .collect();
        v.sort_by(|a, b| {
            a.order
                .unwrap_or(999)
                .cmp(&b.order.unwrap_or(999))
                .then_with(|| a.label.cmp(&b.label))
        });
        v
    })
}

/// All tools, in registry order (wired install-menu order first, then by label).
/// Part of the loader's public surface (mirrors the frontend's full list); kept
/// even though the backend currently reaches tools via `get`/`wired`.
#[allow(dead_code)]
pub fn all() -> &'static [ToolMeta] {
    registry().as_slice()
}

/// Look up a tool by its camelCase id.
pub fn get(id: &str) -> Option<&'static ToolMeta> {
    registry().iter().find(|t| t.id == id)
}

/// Iterator over the wired (installable) tools.
pub fn wired() -> impl Iterator<Item = &'static ToolMeta> {
    registry().iter().filter(|t| t.wired)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_loads_and_has_wired_tools() {
        assert!(all().len() >= 7, "expected the bundled tool set to load");
        // The seven first-class install targets must be present + wired.
        for id in ["claudeCode", "codex", "geminiCli", "copilot", "qwen", "cursor", "opencode"] {
            let m = get(id).unwrap_or_else(|| panic!("missing tool {id}"));
            assert!(m.wired, "{id} should be wired");
            assert!(m.format.is_some(), "{id} needs a render format");
            assert!(m.dest.is_some(), "{id} needs dest templates");
        }
    }

    #[test]
    fn recognized_tools_are_not_wired() {
        for id in ["windsurf", "aider", "openclaw", "antigravity", "kimi"] {
            assert!(!get(id).unwrap().wired, "{id} should be recognized-only");
        }
    }
}
