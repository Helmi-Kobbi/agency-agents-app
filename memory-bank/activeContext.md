# Active Context — Agency Agents

**State**: BUILD. Phases 0–3 DONE. Brew swept. Icon + About done. **Adopt→Track safety fix DONE.
#1 clone-as-source-of-truth (slices 1–4) DONE — not yet run by Michael.**
**Last updated**: 2026-06-06

## Just landed (this session)
- **Adopt → Track**: destructive Adopt gone. `track_agent` records provenance, writes nothing; every
  write backs up first (`<app_data>/backups/`); `agent_diff` for review-before-Update. UI:
  Foreign→"Track", modified→"Restore", Dashboard "found to track".
- **#1 slice 1 — categories from tooling**: `discover_categories` parses `AGENT_DIRS` from
  `scripts/convert.sh`. **Data fix: `integrations` (convert.sh output) dropped (210→209); `strategy`
  added.** Removed the orphan `integrations/backend-architect-with-memory.md` from the baseline (it's
  a valid-but-misfiled enrichment example; to ship it for real, promote it UPSTREAM into a real
  category — then it flows in via refresh).
- **#1 slices 2–4 — catalog source**: `CatalogSource` (Bundled | Managed{~/.agency-agents} |
  UserClone{path,manage}) in `state/catalog.json`; corpus reads/writes the RESOLVED root. Detect
  (~/.agency-agents + "Find" scan), provision (git clone or snapshot), pull (git pull or tarball).
  First-run picker (`CatalogFirstRun`) + `Settings → Catalog`. Verbs Track/Update, manage-with-
  permission, picker+Find — all as decided. cargo test 275/0; svelte-check 0 err; build green.
- ⚠️ NOTE: existing installs (incl. Michael's) have no catalog.json → the **first-run picker WILL
  appear** on next launch (by design — one-time source choice; pick "Bundled" to keep current).

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

## Immediate next: Michael runs it, then #2 / #3
**#1 slices 1–4 ✅ done.** Remaining for #1 (deferred refinements, non-blocking):
- `aliases.json` (slug renames across catalog versions) — not yet honored.
- Explicit **orphan** surfacing (ledger rows whose slug left the catalog) + unique-slug enforcement.
- `.agency-cache/` convention + add to the agency-agents repo `.gitignore` (cache not yet written).
- Symlink-aware reconcile (the `~/.claude` alias case) — still the old behavior.

Then: **#2 Track-all / Update-all**; **#3 tool-grouped Library IA** (L1 tools+counts → L2 per-tool)
+ wire `agent_diff` into a review-before-Update UI.

## Decisions locked (this session)
- Build order: **Both, Track first** → Track DONE, now #1.
- Clone detection: **picker-primary + a "Find Agency Agents" button** (opt-in scan, not auto).
- Existing clone: **manage-with-permission**. Managed path: **`~/.agency-agents`**.
- Cache dir: `.agency-cache/`. Verbs: **Track / Update**.
- Categories: **parse from repo tooling** (`AGENT_DIRS` in convert.sh), not a frontmatter heuristic.

## ✅ RESOLVED: "Adopt" is no longer unsafe
Adopt → **Track** (non-destructive) + backup-on-write shipped this session. The old clobber path is
gone.
