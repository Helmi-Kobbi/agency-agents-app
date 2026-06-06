# NEXT SESSION — resume notes (Agency Agents)

Read this first after a compaction. Then `activeContext.md`, `phases/phase-roadmap.md` (v2 block),
`contracts.md`, `systemPatterns.md`, `decisions.md`. Append-only history is in `agentLog.md`.

## TL;DR
Native macOS app (Tauri 2 · Svelte 5 runes · Rust) — "app store for AI agents," forked from
brew-browser. Browses the agency-agents catalog (210 agents/16 cats) and installs/tracks agents
across AI tools (Claude Code, Cursor, Codex, Gemini CLI, Copilot, opencode, qwen). The app IS the
cross-tool install registry. **Phases 0–3 done. Brew domain swept. New icon + About done. First git
commit made. NEXT = #1 clone-as-source-of-truth.**

## How to run / critical env facts
- `npm run tauri dev` (repo root). **DEV PORT = 1430** (HMR 1431). brew-browser uses 1420 — do NOT
  share, or one app's webview loads the other's frontend.
- Opens on **Agents** (personas). Default set in `src/lib/stores/ui.svelte.ts` (`section = $state("personas")`).
- Read-only reference clones: `/tmp/brew-browser-inspect` (structural fork source),
  `/tmp/agency-agents-inspect` (the catalog + `scripts/convert.sh` renderer spec + `scripts/install.sh`).
- App data: `~/Library/Application Support/com.zerologic.agency-agents-app/` → `corpus/` (seeded
  agents), `state/{corpus-index.json,corpus-meta.json,installs.json (ledger)}`, `settings.json`.
- Verify green: `cd src-tauri && cargo test --lib` (~265/0); `npm run build`; `npm run check`.

## What's built (works)
- Corpus subsystem (parse, sha256 split-hash index, GitHub-tarball refresh), Agents catalog.
- Install + reconcile: `render/` (7 tools, deterministic, ports convert.sh), `install/` (ledger,
  5-state `classify`, tools detect, projects, loadout export/import), commands wired in `lib.rs`.
- Frontend: PersonaDiscover (Agents), AgentLibrary (Library), ToolsView, Loadouts, AgencyDashboard,
  Sidebar, Settings shell, Activity, About. Stores: corpus, install, ui, settings, toast, activity,
  updater, github.
- New brain-circuit app icon (dark shipped). About rebranded.

## The plan (v2 — see phase-roadmap.md for detail)
0. ✅ Brew-domain sweep (done)
1. **Clone-as-source-of-truth** ← NEXT: detect/prompt existing clone OR provision `~/.agency-agents`;
   pull-on-first-launch; **dynamic category discovery** (drop hardcoded `CATEGORY_DIRS` in
   corpus/mod.rs — a division = any top-level dir with agent-frontmatter files); cache in
   `.agency-cache/` inside the clone (add to agency-agents repo .gitignore); `aliases.json` for
   renames; surface **orphans**; enforce unique slugs. First-run **selection modal** + Settings →
   Catalog source.
2. **Track-all / Update-all** (trustworthy after #1's pull).
3. **Tool-grouped Library IA** (L1 tools+counts+state → L2 per-tool agents) + **safe verbs**
   (Track=non-destructive / Update=diff+backup / catalog-drift guard) + **symlink-aware reconcile**
   (`Linked` state; NEVER write through a symlink; recurse subdirs; recognize whole-dir aliases).
   Library & Tools stay SEPARATE (user's call).
4. **Renderer-parity test** vs scripts/convert.sh.
5. Phase 4 Trending+GitHub · Phase 5 Quality · Phase 6 Release (signed/notarized DMG, updater key).
   Deferred: 4 multi-file renderers (antigravity/openclaw/aider/windsurf) = task #8.

## Open confirmations (ask before #1)
(1) clone detection auto-find+picker vs picker-only; (2) existing clone read-only vs manage-with-
permission (user leaned manage-with-permission); (3) cache dir `.agency-cache/`; (4) verbs Track/Update.

## ⚠️ KNOWN ISSUES / fix early
- **"Adopt" is still UNSAFE**: `install/mod.rs::adopt_agent` = do_install = re-render catalog version
  + OVERWRITE file. Can downgrade/clobber a CLI-managed agent. Replace with **Track** (record on-disk
  hash, NO write) + explicit **Update** (diff+backup). User keeps flagging it — consider doing this
  first/soon even before full #3.
- **Settings still has brew remnants** (kept intentionally as "infra"): `SettingsSectionNetwork`
  brew-flavored controls, `CaskIconMode`/`CatalogAutoRefresh`/`trendingTtl` on the Settings type,
  `BrewError` name everywhere, Pill `formula`/`cask` tones, brew strings in tokens.css/comments.
  Second-pass polish; non-blocking. `BrewError → AppError` rename is wide; do as a deliberate pass.

## GOTCHAS / hard-won lessons (don't relearn these)
- **Frontend "frozen/blank" bug → open the webview devtools console FIRST** (right-click → Inspect).
  The multi-hour "Library frozen" saga was a `each_key_duplicate` (Copilot dual-writes ~/.github +
  ~/.copilot → two rows, same `{#each}` key) that threw and halted the component — looked exactly
  like dead reactivity. Fixed by keying `{#each}` on `r.dest` + backend dedupe by (slug,tool,project)
  in installs_reconcile. Keys must be unique; prefer a path/dest over composite business keys.
- **Reactivity rule we adopted**: view components are PURE READERS of stores; the global reconcile is
  triggered ONCE in `+layout.svelte` onMount, and Rescan buttons re-trigger on user action. Calling a
  store mutation during a component's own setup can wedge it. AgentLibrary snapshots install.installed
  into one local via a single read.
- **Icon won't update in dev?** `tauri dev` incremental builds DON'T re-embed a changed icns →
  `touch src-tauri/build.rs`, rebuild, `killall Dock`. The .icns is correct; it's an embed/cache issue.
- **I (the agent) cannot reliably self-screenshot the app**: terminal holds focus (region capture
  grabs the terminal) and the transparent WKWebView blanks on window-id capture. **Ask Michael for
  screenshots**, or use the devtools console. (Memory: [[screenshot-app-window-only]].)
- **Dev Dock label = binary name** "agency-agents-app"; menu bar = productName "Agency Agents". Only a
  `tauri build` (real .app) makes them match. Don't rename the binary (no spaces in binaries).
- **Catalog = 210 agent personas** (files with `name:` frontmatter), NOT 251. strategy/ + examples/
  are docs (NEXUS playbooks / workflow examples) → candidate future "Playbooks" section.
- **Renderer parity**: our Rust `render/` output must byte-match `scripts/convert.sh` or CLI-installed
  transform-tool files falsely read as "Modified" (#4 in plan).

## Posture (locked)
The app is a **respectful frontend, never an authority**: never auto-mutate; default read-only; all
writes explicit/previewed/reversible; source of truth = the user's clone, shared with the CLI.

## Key files
- Backend: `src-tauri/src/{corpus,render,install,github,util}/`, `state.rs`, `types.rs`, `error.rs`,
  `lib.rs`. Tauri config: `src-tauri/tauri.conf.json` (id com.zerologic.agency-agents-app, port 1430,
  macOSPrivateApi true, transparent true).
- Frontend: `src/lib/components/{PersonaDiscover,AgentLibrary,ToolsView,Loadouts,AgencyDashboard,
  Sidebar,Settings,AboutModal}.svelte`, `src/lib/stores/{corpus,install,ui,settings}.svelte.ts`,
  `src/routes/+page.svelte` (router, no {#key}), `+layout.svelte` (global reconcile).
- Icons: `docs/icon/agency-icon-{dark,light}-1024.png`, `src/lib/assets/app-icon.png`, `src-tauri/icons/*`.
