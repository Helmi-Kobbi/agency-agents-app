# Active Context — Agency Agents

**State**: BUILD. Phases 0 · 1 · 1.5 · 2 (+follow-ups) · 3 DONE. Brew-domain swept. New app icon +
About rebranded. **Next: #1 — clone-as-source-of-truth.**
**Last updated**: 2026-06-06

> Full plan + sequence: `phases/phase-roadmap.md` (the "v2" block). Detailed resume notes +
> gotchas: `NEXT-SESSION.md`. Build spec: `contracts.md`. Architecture: `systemPatterns.md`.

## How to run (dev)
- `npm run tauri dev` from repo root. **Dev server is on port 1430** (NOT 1420 — that's
  brew-browser; sharing it makes one app load the other's frontend). HMR for frontend; Rust changes
  recompile. The app opens on **Agents** (personas).
- Reference clones (read-only): `/tmp/brew-browser-inspect`, `/tmp/agency-agents-inspect`.

## What works (verified)
- **Agents** catalog (210 agents / 16 categories), search, persona detail with an **Install** menu.
- **Library** — flat list of installs; your ~184 `install.sh` agents show as `foreign` with Adopt.
- **Tools**, **Loadouts** (Agentfile), **Dashboard** (agency rollup), Activity, Settings (⌘,).
- Backend: `corpus · render · install · github · util · commands{github,settings,updater}`.
  `cargo test` ~265/0; `vite build` + `svelte-check` green; app boots clean (210 corpus seeded).
- New brain-circuit **app icon** (dark shipped; light master in `docs/icon/`). About window rebranded.

## Immediate next: #1 clone-as-source-of-truth
Detect/prompt for an existing agency-agents clone OR provision `~/.agency-agents`; **pull on first
launch**; **dynamic category discovery** (drop hardcoded `CATEGORY_DIRS`); cache in `.agency-cache/`
(add to agency-agents repo `.gitignore`); honor `aliases.json` (renames); surface **orphans**;
enforce unique slugs. First-run **selection modal** + **Settings → Catalog source**.

## Open confirmations (still unanswered — ask before building #1)
1. Clone detection: auto-find (`~/.agency-agents` + scan common dev dirs) + picker, OR just picker?
2. Existing clone: read-only default + opt-in "let the app pull it"?  (User leaned: manage-with-permission.)
3. Cache dir name `.agency-cache/`?
4. Verbs: **Track** (non-destructive) + **Update** (diff+backup) — keep these words?

## ⚠️ KNOWN ISSUE to fix early
**"Adopt" is still the OLD unsafe action** — `adopt_agent` re-renders the catalog version and
OVERWRITES the on-disk file (can downgrade/clobber a CLI-managed agent). Agreed replacement: rename
**Adopt → Track** (record on-disk hash, NO write) + explicit **Update** (diff+backup). NOT built yet
(Step 3). User keeps noticing it; consider pulling the safe Track rename forward.
