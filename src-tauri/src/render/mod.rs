//! Deterministic per-tool agent renderers + destination-path resolution.
//!
//! Ports agency-agents `scripts/convert.sh`. Every renderer is a PURE function
//! of `(Agent, raw source)` — no timestamps, no randomness, stable key order —
//! so `rendered_hash` is reproducible. That reproducibility is the load-bearing
//! requirement for install-state reconciliation (`reconcile/`): we identify an
//! installed file as "ours" by re-rendering its slug for its tool and matching
//! bytes. See `memory-bank/contracts.md` §B/§E.
//!
//! Identity tools (claude-code, copilot) ship the agent `.md` verbatim, so their
//! "render" is the raw corpus source. Transform tools (cursor/.mdc, codex/TOML,
//! gemini-cli, opencode, qwen) rebuild the file from frontmatter fields + body.
//! The remaining tools (antigravity skill dirs, openclaw multi-file, aider /
//! windsurf accumulated files) are special multi-file shapes — not yet supported
//! here; `render`/`dests` return an error so the UI can disable them cleanly.

use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::BrewError;
use crate::types::{Agent, Scope, Tool};

impl Tool {
    /// User-global (`~/…`) vs project-scoped (`./…`) deployment.
    pub fn scope(self) -> Scope {
        match self {
            Tool::Cursor | Tool::Opencode | Tool::Windsurf | Tool::Aider => Scope::Project,
            _ => Scope::User,
        }
    }

    /// Whether Phase 2 can render+install this tool. The unsupported four are
    /// multi-file / accumulated shapes deferred to a later phase.
    pub fn supported(self) -> bool {
        matches!(
            self,
            Tool::ClaudeCode
                | Tool::Copilot
                | Tool::Cursor
                | Tool::Codex
                | Tool::GeminiCli
                | Tool::Opencode
                | Tool::Qwen
        )
    }

    /// kebab id, matching `scripts/install.sh` tool names.
    pub fn id(self) -> &'static str {
        match self {
            Tool::ClaudeCode => "claude-code",
            Tool::Copilot => "copilot",
            Tool::Cursor => "cursor",
            Tool::GeminiCli => "gemini-cli",
            Tool::Codex => "codex",
            Tool::Opencode => "opencode",
            Tool::Windsurf => "windsurf",
            Tool::Aider => "aider",
            Tool::Qwen => "qwen",
            Tool::Openclaw => "openclaw",
            Tool::Antigravity => "antigravity",
        }
    }

    /// Human label for the UI.
    pub fn label(self) -> &'static str {
        match self {
            Tool::ClaudeCode => "Claude Code",
            Tool::Copilot => "GitHub Copilot",
            Tool::Cursor => "Cursor",
            Tool::GeminiCli => "Gemini CLI",
            Tool::Codex => "Codex",
            Tool::Opencode => "opencode",
            Tool::Windsurf => "Windsurf",
            Tool::Aider => "Aider",
            Tool::Qwen => "Qwen Code",
            Tool::Openclaw => "OpenClaw",
            Tool::Antigravity => "Antigravity",
        }
    }
}

/// SHA-256, lowercase hex — the canonical hash for the ledger + reconcile.
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let d = h.finalize();
    let mut s = String::with_capacity(64);
    for b in d {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn unsupported(tool: Tool) -> BrewError {
    BrewError::Io {
        message: format!(
            "tool '{}' is not supported for install yet (multi-file format)",
            tool.id()
        ),
    }
}

/// Render the file content for `tool` from `agent` (+ the raw corpus `.md`
/// source, used verbatim by identity tools). Deterministic.
pub fn render(agent: &Agent, raw_source: &str, tool: Tool) -> Result<String, BrewError> {
    let body = agent.body.as_str();
    let out = match tool {
        // Identity — ship the corpus `.md` exactly as authored.
        Tool::ClaudeCode | Tool::Copilot => raw_source.to_string(),

        // Cursor `.mdc`: description + globs + alwaysApply frontmatter.
        Tool::Cursor => format!(
            "---\ndescription: {desc}\nglobs: \"\"\nalwaysApply: false\n---\n{body}\n",
            desc = agent.description,
        ),

        // Codex TOML: minimal required fields, control chars escaped.
        Tool::Codex => format!(
            "name = \"{name}\"\ndescription = \"{desc}\"\ndeveloper_instructions = \"{body}\"\n",
            name = toml_escape(&agent.name),
            desc = toml_escape(&agent.description),
            body = toml_escape(body),
        ),

        // Gemini CLI subagent `.md`: name(=slug) + description frontmatter.
        Tool::GeminiCli => format!(
            "---\nname: {slug}\ndescription: {desc}\n---\n{body}\n",
            slug = agent.slug,
            desc = agent.description,
        ),

        // Qwen Code SubAgent `.md`: same shape as Gemini CLI.
        Tool::Qwen => format!(
            "---\nname: {slug}\ndescription: {desc}\n---\n{body}\n",
            slug = agent.slug,
            desc = agent.description,
        ),

        // OpenCode `.md`: name + description + mode + hex color frontmatter.
        Tool::Opencode => format!(
            "---\nname: {name}\ndescription: {desc}\nmode: subagent\ncolor: '{color}'\n---\n{body}\n",
            name = agent.name,
            desc = agent.description,
            color = resolve_opencode_color(agent.color.as_deref().unwrap_or("")),
        ),

        Tool::Windsurf | Tool::Aider | Tool::Openclaw | Tool::Antigravity => {
            return Err(unsupported(tool))
        }
    };
    Ok(out)
}

/// Render + hash in one shot.
pub fn render_with_hash(
    agent: &Agent,
    raw_source: &str,
    tool: Tool,
) -> Result<(String, String), BrewError> {
    let bytes = render(agent, raw_source, tool)?;
    let hash = sha256_hex(bytes.as_bytes());
    Ok((bytes, hash))
}

/// Absolute destination path(s) for an installed agent. Most tools write a
/// single file; Copilot dual-writes to `~/.github` and `~/.copilot`.
///
/// `home` is the user's home dir (user-scoped tools). `project_root` is required
/// for project-scoped tools (cursor, opencode) and ignored otherwise.
pub fn dests(
    tool: Tool,
    slug: &str,
    home: &Path,
    project_root: Option<&Path>,
) -> Result<Vec<PathBuf>, BrewError> {
    let proj = || -> Result<&Path, BrewError> {
        project_root.ok_or_else(|| BrewError::Io {
            message: format!("tool '{}' is project-scoped; a project path is required", tool.id()),
        })
    };
    let v = match tool {
        Tool::ClaudeCode => vec![home.join(".claude/agents").join(format!("{slug}.md"))],
        Tool::Copilot => vec![
            home.join(".github/agents").join(format!("{slug}.md")),
            home.join(".copilot/agents").join(format!("{slug}.md")),
        ],
        Tool::Codex => vec![home.join(".codex/agents").join(format!("{slug}.toml"))],
        Tool::GeminiCli => vec![home.join(".gemini/agents").join(format!("{slug}.md"))],
        Tool::Qwen => vec![home.join(".qwen/agents").join(format!("{slug}.md"))],
        Tool::Cursor => vec![proj()?.join(".cursor/rules").join(format!("{slug}.mdc"))],
        Tool::Opencode => vec![proj()?.join(".opencode/agents").join(format!("{slug}.md"))],
        Tool::Windsurf | Tool::Aider | Tool::Openclaw | Tool::Antigravity => {
            return Err(unsupported(tool))
        }
    };
    Ok(v)
}

/// Map an agency-agents `color` (named or hex) to an OpenCode-safe `#RRGGBB`
/// (uppercase). Unknown → neutral gray. Ported from `resolve_opencode_color`.
fn resolve_opencode_color(color: &str) -> String {
    let c = color.trim().to_ascii_lowercase();
    let mapped = match c.as_str() {
        "cyan" => "#00FFFF",
        "blue" => "#3498DB",
        "green" => "#2ECC71",
        "red" => "#E74C3C",
        "purple" => "#9B59B6",
        "orange" => "#F39C12",
        "teal" => "#008080",
        "indigo" => "#6366F1",
        "pink" => "#E84393",
        "gold" => "#EAB308",
        "amber" => "#F59E0B",
        "neon-green" => "#10B981",
        "neon-cyan" => "#06B6D4",
        "metallic-blue" => "#3B82F6",
        "yellow" => "#EAB308",
        "violet" => "#8B5CF6",
        "rose" => "#F43F5E",
        "lime" => "#84CC16",
        "gray" => "#6B7280",
        "fuchsia" => "#D946EF",
        other => other,
    };
    let hex = mapped.strip_prefix('#').unwrap_or(mapped);
    let is_hex6 = hex.len() == 6 && hex.bytes().all(|b| b.is_ascii_hexdigit());
    if is_hex6 {
        format!("#{}", hex.to_ascii_uppercase())
    } else {
        "#6B7280".to_string()
    }
}

/// Escape a value for a TOML basic string (ported from `toml_escape_string`).
fn toml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || (c as u32) == 0x7F => {
                out.push_str(&format!("\\u{:04X}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agent() -> Agent {
        Agent {
            slug: "frontend-developer".into(),
            name: "Frontend Developer".into(),
            description: "Builds UIs.".into(),
            category: "engineering".into(),
            emoji: Some("🎨".into()),
            color: Some("blue".into()),
            vibe: Some("Ships pixels.".into()),
            body: "You are a frontend dev.\n".into(),
        }
    }

    #[test]
    fn claude_code_is_identity() {
        let a = agent();
        let raw = "---\nname: Frontend Developer\n---\nORIGINAL BODY\n";
        assert_eq!(render(&a, raw, Tool::ClaudeCode).unwrap(), raw);
        assert_eq!(render(&a, raw, Tool::Copilot).unwrap(), raw);
    }

    #[test]
    fn cursor_mdc_shape() {
        let out = render(&agent(), "", Tool::Cursor).unwrap();
        assert!(out.starts_with("---\ndescription: Builds UIs.\nglobs: \"\"\nalwaysApply: false\n---\n"));
        assert!(out.contains("You are a frontend dev."));
    }

    #[test]
    fn codex_toml_escapes() {
        let mut a = agent();
        a.description = "has \"quotes\" and\nnewline".into();
        let out = render(&a, "", Tool::Codex).unwrap();
        assert!(out.contains("description = \"has \\\"quotes\\\" and\\nnewline\""));
        assert!(out.starts_with("name = \"Frontend Developer\""));
    }

    #[test]
    fn opencode_color_maps_to_hex() {
        let out = render(&agent(), "", Tool::Opencode).unwrap();
        assert!(out.contains("color: '#3498DB'"), "blue → #3498DB: {out}");
        assert!(out.contains("mode: subagent"));
    }

    #[test]
    fn opencode_unknown_color_falls_back() {
        let mut a = agent();
        a.color = None;
        let out = render(&a, "", Tool::Opencode).unwrap();
        assert!(out.contains("color: '#6B7280'"));
    }

    #[test]
    fn gemini_uses_slug_as_name() {
        let out = render(&agent(), "", Tool::GeminiCli).unwrap();
        assert!(out.starts_with("---\nname: frontend-developer\ndescription: Builds UIs.\n---\n"));
    }

    #[test]
    fn render_is_deterministic() {
        for tool in [Tool::Cursor, Tool::Codex, Tool::Opencode, Tool::GeminiCli, Tool::Qwen] {
            let a = render(&agent(), "raw", tool).unwrap();
            let b = render(&agent(), "raw", tool).unwrap();
            assert_eq!(a, b, "{tool:?} must be deterministic");
        }
    }

    #[test]
    fn unsupported_tools_error() {
        for tool in [Tool::Windsurf, Tool::Aider, Tool::Openclaw, Tool::Antigravity] {
            assert!(render(&agent(), "raw", tool).is_err());
            assert!(dests(tool, "x", Path::new("/home"), Some(Path::new("/p"))).is_err());
        }
    }

    #[test]
    fn dests_per_tool() {
        let home = Path::new("/Users/x");
        let proj = Path::new("/proj");
        assert_eq!(
            dests(Tool::ClaudeCode, "a", home, None).unwrap(),
            vec![PathBuf::from("/Users/x/.claude/agents/a.md")]
        );
        assert_eq!(dests(Tool::Copilot, "a", home, None).unwrap().len(), 2);
        assert_eq!(
            dests(Tool::Codex, "a", home, None).unwrap(),
            vec![PathBuf::from("/Users/x/.codex/agents/a.toml")]
        );
        assert_eq!(
            dests(Tool::Cursor, "a", home, Some(proj)).unwrap(),
            vec![PathBuf::from("/proj/.cursor/rules/a.mdc")]
        );
        // project-scoped without a project path → error
        assert!(dests(Tool::Cursor, "a", home, None).is_err());
    }

    #[test]
    fn scope_classification() {
        assert_eq!(Tool::ClaudeCode.scope(), Scope::User);
        assert_eq!(Tool::Cursor.scope(), Scope::Project);
        assert_eq!(Tool::Opencode.scope(), Scope::Project);
        assert_eq!(Tool::Codex.scope(), Scope::User);
    }
}
