# Agent Log — Agency Agents (append-only)

2026-06-05 — Phase 0 (lead): recon of agency-agents (107.5K★, 251 agents/18 cats) +
brew-browser (Tauri2/Svelte5/Rust, 73 cmds). Locked 1:1 architecture. Forked scaffold,
rebranded to Agency Agents. Green cargo check + vite build. Wrote memory-bank.
Next: Phase 1 team build (Corpus + Discover).

2026-06-05 — Phase 1 (team wh3t9mkee + lead-as-integrator): Types agent landed types.rs+types.ts
(+5 wire-format tests). Corpus agent landed corpus/mod.rs+parse.rs (frontmatter parse, sha256
split-hash index, corpus_* commands, tarball refresh) + bundled baseline + Cargo deps
(serde_yaml/sha2/hex/flate2/tar/tracing) + tauri.conf resources. Discover agent landed
corpus.svelte.ts store + PersonaDiscover.svelte + nav wiring. Workflow threw on a StructuredOutput
protocol error AFTER the code compiled, so the Integrate phase didn't run — lead took over as
integrator: fixed rebrand keychain id (brew-browser→agency-agents-app) + its 2 tests, decoupled
corpus category metadata onto its own data/agency-categories.json (was colliding with brew's
categories.json), restored brew's real bundled data (placeholders had broken brew's tests), added a
real-baseline parse test. THAT test caught the big one: seeding undercounted (flat layout missed
nested game-dev/strategy agents) AND the true agent count is 210, not 251 — strategy/ (NEXUS
playbooks) + examples/ (workflow docs) have no agent frontmatter, so they're documentation, not
agents. Corrected CATEGORY_DIRS 18→16, baseline re-seeded recursively to 210 agents / 16 categories.
Verdict: cargo check GREEN, vite build GREEN, cargo test 615 passed / 0 failed. Phase 1 DONE.
Next: Phase 2 (Install + Reconcile).

2026-06-05 — Phase 1.5 (lead): agency-first polish. Added 12 Lucide category icons (killed "?"
fallbacks); rewrote Sidebar lean (🤖 brand + Agents/Activity nav, dropped brew package-search +
brew env footer, added live agent-count footer); default landing dashboard→personas; enabled
macOSPrivateApi vibrancy. Verified live (window-only screenshot). 615 tests stay green.

2026-06-05 — Phase 2 (lead, drove solo): Install + Reconcile.
- 2a render/: deterministic per-tool converters ported from convert.sh — claude-code/copilot
  (identity), cursor(.mdc), codex(TOML+escape), gemini-cli, qwen, opencode(color→hex). dests() per
  tool/scope; Copilot dual-write; 4 unsupported tools (antigravity/openclaw/aider/windsurf) error
  cleanly. 10 tests.
- 2b install/: ledger (installs.json), reconcile classify() → 5 states, tools detection, projects
  registry. Commands: install/update/adopt/uninstall_agent, installs_reconcile, installs_for_agent,
  tools_list, projects_list. Exposed corpus helpers (entry/version/read_source, pub(crate) paths).
  End-to-end tests (write→disk→reconcile through all 5 states; identity verbatim; project-root write).
- 2c UI: install store (install.svelte.ts), persona-detail Install menu (user tools 1-click;
  project tools folder-pick via dialog), installed-state chips with status + uninstall, toasts.
Verdict: cargo test 630 passed / 0 failed; vite GREEN; app launches healthy (210 agents, no panic).
Remaining for Phase 2: dedicated Library + Tools nav sections (install flow itself is done in the
persona panel). Next: Library/Tools views, then Phase 3 (Loadouts + Dashboard).

2026-06-05 — Phase 2 follow-ups (lead): (1) update_kind — added body_hash to InstallRecord
(#[serde(default)] for back-compat), reconcile now labels Outdated installs Cosmetic vs Substantive.
(2) Foreign sweep — installs_reconcile scans each tool's dir(s) (user + ledger project dirs) for
recognized-but-unledgered corpus agents → Foreign state (Adopt-able). (3) Library view
(AgentLibrary.svelte) — reconciled cross-tool installs, attention-first, status chips, per-state
actions (Update/Reinstall/Adopt/Remove). (4) Tools view (ToolsView.svelte) — detected tools + scope
+ counts + dest. Wired both into sidebar nav (Agents/Library/Tools/Activity) + fixed ⌘1/2/3/6 keymap
to match. Library badge shows attention count. cargo test 630 / 0; vite GREEN. Deferred (noted in
install/mod.rs): the 4 multi-file renderers (antigravity/openclaw/aider/windsurf). Next: Phase 3.
NB: real-world validation — Michael ran the repo's install.sh (184 agents → ~/.claude/agents); the
Foreign sweep correctly surfaced ~180 as Adopt-able in the Library. Zero fake data.

2026-06-05 — Phase 3 (lead): Loadouts + Dashboard.
- Loadouts: loadout_export/import commands (Agentfile JSON manifest; export reads ledger, import
  installs each entry, skips failures). Loadouts.svelte (Export/Restore via dialog save/open +
  current-loadout list). agentfile_roundtrips test.
- Dashboard: AgencyDashboard.svelte rollup (available / installed-by-you / need-attention /
  found-to-adopt stat tiles → deep-link to Library; catalog-by-category bars; detected-tools strip).
  Replaced brew Dashboard in routing.
- Nav now full agency: Dashboard/Agents/Library/Tools/Loadouts/Activity (⌘0-4,6 keymap aligned).
- cargo test 631 / 0; vite GREEN. Phase 3 DONE. Next: Phase 4 (Trending + GitHub).

2026-06-05 — Bugfix (lead): "Library panel broken" + Tools rescan.
- ROOT CAUSE (proved via temp DBG readout): components called `install.reconcile()` / `loadTools()`
  at SCRIPT TOP LEVEL, mutating install-store $state (reconciling/installed) DURING the component's
  own setup/derivation → froze that component's reactivity. Backend was fine (reconcile logged
  "DONE: 190 rows"); the Sidebar (read-only) updated to 190 while AgentLibrary stayed frozen at
  installed=0/reconciling=true → header/body/badge disagreed.
- FIX: moved all reconcile()/loadTools()/ensureLoaded() side-effects into onMount() in AgentLibrary,
  ToolsView, Loadouts, AgencyDashboard, PersonaDiscover. Replaced the store's class #private
  in-flight guard with a module-level promise (class #private fields can trip Svelte 5's $state
  transform). Added reconciling/reconciled flags + a real LoadingState in the Library; emoji lookup
  via a Map (was O(n) per row × 180+). Null-safe row sort. Dashboard <li role=button> → <button>.
- ADDED: Tools view "Rescan" button (re-detect tools + re-reconcile) — the requested Tool Update.
- cargo test 631/0; vite GREEN. NOTE: couldn't visually confirm in-session — screen capture is
  blocked (terminal holds focus; transparent webview blanks on occluded window-id capture). Needs
  Michael to confirm the Library populates (it should now show the ~190 Foreign agents).

2026-06-06 — Library "frozen" bug RESOLVED (lead, via Michael's devtools console). ROOT CAUSE was
NOT reactivity (hours lost chasing that): a Svelte `each_key_duplicate` crash. The `{#each rows}` key
was `slug+tool+projectPath`; **Copilot dual-writes to ~/.github AND ~/.copilot**, so the Foreign
sweep produced TWO identical-key rows → Svelte threw during render → AgentLibrary's whole reactive
subtree halted and froze on the last good frame (loading skeleton), reverting all reads to 0 → which
LOOKED exactly like a reactivity freeze. LESSON: for a frontend frozen/blank bug, open the webview
devtools console FIRST (right-click → Inspect in Tauri dev) — it named the cause in one line. FIX:
(1) `{#each rows as r (r.dest)}` (unique per file); (2) backend `installs_reconcile` dedupes by
(slug, tool, project) so Copilot is one logical row. The single-`$effect` snapshot refactor was kept
(good hygiene) but was NOT the fix. Library now renders 185 installs / 184 foreign with Adopt
buttons. Debug tags removed; default landing restored to Agents. Next: tool-grouped Library IA +
Track/Update safety + clone-as-source-of-truth.

2026-06-06 — Brew-domain sweep DONE (delegated to a focused agent w/ exact keep/remove spec, lead
verified). Removed the whole inherited brew domain: backend dirs brew/catalog/enrichment/trending/
vulns + 16 brew command files + brew state fields/init + brew data + brew types; frontend brew
components (Dashboard/Library/Discover/Snapshots/Services/PackageDetail/Row/Trending/Upgrade/Issue)
+ 4 brew Settings sections + ~14 brew stores + brew api.ts/types.ts + +page brew branches +
CommandPalette/ui trims. KEPT (forward infra, not-yet-wired): Settings shell + generic sections,
full design system, updater, github/ (→P4), util, the agency domain. Smart relocations: SSRF guard
`is_public_host` → util/net.rs (used by github/url.rs); `app_version` → commands/settings.rs.
Verified: backend = corpus/render/install/github/util/commands{github,settings,updater}; cargo test
265/0; vite + svelte-check GREEN; app boots clean (210 corpus seeded, no panic). Also fixed
EmptyState to accept `children` (agency empty states now render their copy). Second-pass polish
deferred (not blocking): rename BrewError→AppError; agency-ify Settings Network/CaskIcon fields;
Pill formula/cask tones; brew strings in tokens.css/comments. Next: #1 clone-as-source-of-truth.

2026-06-06 — Adopt→Track SAFETY FIX + #1 slice 1 (dynamic categories) (lead).
- TRACK (was Adopt): `install/mod.rs` — `track_agent`/`do_track` records provenance in the
  ledger but WRITES NOTHING (was do_install = re-render + overwrite). Reconcile then shows the
  tracked file as Current (matches catalog) or Modified (differs) — never clobbered. All writes now
  back up first: `write_agent_files` gained `backup_dir`; `backup_if_differs` copies prior bytes to
  `<app_data>/backups/<file>.<stamp>.bak` (outside tool dirs → invisible to the Foreign sweep) before
  any overwrite, so install/update/restore/import are reversible. New `agent_diff` cmd (+ store
  `diff()`) returns {onDisk,proposed,differs} for review-before-Update. UI: Foreign "Adopt"→"Track";
  modified row now "Restore" via backup-aware update; Dashboard "found to adopt"→"found to track".
  Tests: track_writes_no_file, write_backs_up_existing_differing_file. lib.rs swaps adopt_agent→
  track_agent + agent_diff.
- #1 SLICE 1 — categories now parsed from the REPO TOOLING, not hardcoded (Michael: "categories can
  be parsed from the repo itself, look in the tooling"). `discover_categories(root)` parses the
  `AGENT_DIRS=( … )` bash array from `<root>/scripts/convert.sh` (→ install.sh → lint-agents.sh),
  `parse_agent_dirs` is pure/tested; `DEFAULT_CATEGORIES` is the offline fallback. Threaded through
  resolve_active/build_from_dir/seed_from_baseline/empty_corpus + Corpus.category_order; refresh()
  reads categories from the tarball's own convert.sh (`categories_from_tarball`) and extract writes
  scripts/convert.sh into the working copy so launches stay self-describing. Bundled
  `scripts/convert.sh` into corpus-baseline (ships via existing resource glob).
  DATA CORRECTION: `integrations/` is convert.sh OUTPUT, not a category → dropped (our list wrongly
  had it); `strategy/` IS canonical → added. Net: baseline 210→**209** (the lone integrations file
  backend-architect-with-memory.md is an enrichment artifact, no longer indexed; file still on disk,
  flagged for possible removal). agency-categories.json: −integrations +strategy(Network icon);
  categoryIcon.ts −Plug +Network. Existing installs auto-correct via the DEFAULT fallback (their old
  seeded corpus has no scripts/). Tests updated incl. real-baseline→209, +4 new. Verdict: cargo test
  271/0; svelte-check 0 err; build GREEN. Next: #1 slices 2–4 (CatalogSource model → detect/provision/
  pull → first-run modal + Settings). Managed path = ~/.agency-agents (confirmed).

2026-06-06 — #1 slices 2–4: clone-as-source-of-truth (lead). Catalog now has a SOURCE.
- SLICE 2 — CatalogSource model: `types.rs` enum {Bundled | Managed{path} | UserClone{path,manage}}
  (serde tag "kind"), persisted to `state/catalog.json`. corpus/mod.rs: load/save_catalog_source +
  `catalog_root(app_data,source)` resolver. Refactored resolve_active/read_source/refresh to read the
  RESOLVED root (was hardcoded `<app_data>/corpus`); only Bundled seeds from baseline; refresh refuses
  a read-only UserClone. Cmds: catalog_source_get/set (set validates path + looks_like_catalog, then
  rebuilds+swaps the memoized corpus), catalog_configured (catalog.json exists? → first-run gate).
- SLICE 3 — detect/provision/pull: `detect_catalogs(scan)` always checks ~/.agency-agents, scan=true
  walks ~/{Software,Projects,git,Developer,code,dev,src} for an agency-agents checkout; returns
  CatalogCandidate{path,kind,has_git,agent_count}. `provision_managed` = git clone --depth1 (if git on
  PATH) else snapshot tarball into ~/.agency-agents. `pull_active` = git pull --ff-only (git checkout)
  else tarball refresh. git via spawn_blocking(std::process). Cmds: catalog_detect,
  catalog_provision_managed, catalog_pull (all require_network where they hit the net). Factored
  download_corpus_tarball + rebuild_corpus helpers.
- SLICE 4 — frontend: `stores/catalog.svelte.ts` (source/configured/detection/busy + load/detect/
  setSource/useBundled/useClone/provisionManaged/pull, calls corpus.reload() after switches; added
  corpus.reload()). `CatalogFirstRun.svelte` (first-launch picker: use my clone[Find+folder picker,
  manage checkbox] / set up ~/.agency-agents / bundled) rendered from +layout when !configured.
  `SettingsSectionCatalog.svelte` + Settings nav "Catalog" (Library icon) + SettingsSection type:
  shows source/path/agent-count, Pull latest, switch source, Find/pick clone. +layout loads
  catalog on mount.
  Decisions honored: picker-primary + "Find Agency Agents" button; manage-with-permission;
  managed path ~/.agency-agents; verbs Track/Update. Verdict: cargo test 275/0; svelte-check 0 err;
  build GREEN. NOT yet visually run by Michael. Deferred refinements: aliases.json (renames),
  explicit orphan surfacing, symlink-aware reconcile (those were #2/#3 in the old plan; core source
  switching works). Next: Michael runs it; then #2 Track-all/Update-all, #3 tool-grouped Library IA.

2026-06-06 — Warning purge + AppError rename + Catalog GitHub/sync (lead). Michael flagged
dead-code warnings on `tauri dev` + asked for repo pull/sync management with GitHub login + diff stats.
- CLEANUP (0 warnings now): renamed `BrewError`→`AppError` everywhere (412 refs, sed; Rust-only, wire
  `code` strings unchanged). Pruned dead brew variants (BrewNotFound/BrewExitNonZero/JobNotFound/
  Canceled/BrewfileNotFound/FeatureDisabled/VulnsNotInstalled) + truncate_head/tail + ~30 dead tests;
  removed AppState require_enhanced_trending/live_enrichment/vulnerability_scanning + their tests;
  removed render Tool::supported; removed github resolve_github_homepage (brew-metadata helper) + tests.
  Kept extract_github_repo (now wired). cargo build 0 warnings; cargo test 245/0.
- REAL BUG FIXED: `state.rs::resolve_app_data_dir` pushed "brew-browser" → settings.json + github-cache
  were written to `~/Library/Application Support/brew-browser/` (colliding with the other app), split
  from corpus/ledger/catalog which use the proper bundle dir. Changed to
  "com.zerologic.agency-agents-app" so ALL app data unifies. (Existing users: settings/github-cache
  effectively reset to defaults — acceptable.)
- FEATURE — Catalog GitHub + sync status: backend `catalog_status` (source, git commit/branch/last-
  commit/dirty, remote→repo_slug via extract_github_repo, version/fetchedAt, agentCount) +
  `catalog_check_updates` (git fetch → behind/ahead via rev-list --left-right, `git diff --stat`
  preview + changedFiles). provision_managed now FULL clone (dropped --depth1) for accurate history.
  Frontend: catalog store gains status/updateCheck/loadStatus/checkUpdates (refreshed after pull/
  switch/provision); SettingsSectionCatalog rebuilt to show commit/branch/dirty, "Check for updates"
  → diffstat + Pull CTA, and a GitHub card (repo stars/forks/issues/last-release via the EXISTING
  github.getRepoStats; sign-in via github.signIn → global DeviceFlowModal). Reused all existing github
  infra (auth/device-flow/stats) — no new auth code. svelte-check 0 err; build green.
  Next: Michael runs it (first-run picker appears once); deferred #1 bits (aliases/orphans/.agency-cache/
  symlink reconcile); then #2/#3.

2026-06-06 — Settings panel refocus to Agency Agents' lens (lead). Michael: "we're nowhere near
close :9 look at the setting panel." The whole Settings panel was still a Homebrew control panel.
Refocused every section to show ONLY what's functional + real for Agency Agents:
- NETWORK → "Network & Privacy": removed dead brew controls (Cask icon fetching, Trending TTL,
  Catalog auto-refresh, stale-banner — all non-functional after the brew sweep). Kept the one
  functional toggle (Offline Mode / paranoid_mode, 19 live refs). Rewrote the outbound-paths
  disclosure to the REAL hosts: github.com/codeload (catalog clone/pull/snapshot), api.github.com
  (stats/auth/star), raw/objects.githubusercontent.com (avatars/assets), agency-agents-app.zerologic.com
  (updates). Kept Updates subsection.
- APPEARANCE: removed the brew "AI features" enrichment toggle (aiFeaturesEnabled — 0 live agency
  consumers; gated brew enriched metadata) + its CSS; fixed "launch brew-browser"→"Agency Agents".
- GITHUB: reframed "stats on package pages"→"repository stats" for the catalog repo; de-brewed copy.
- ABOUT: rebranded repo links (app + catalog) + zero-telemetry copy to Agency Agents.
- UPDATES: brew-browser→Agency Agents throughout; updater URL brew-browser.zerologic.com→
  agency-agents-app.zerologic.com/updater.json.
- Visible strings elsewhere: reportIssue (issue URL → agency-agents-app, "Report to Agency Agents"),
  ui.svelte storage key prefix, UpdateIndicator badge, ActivityDrawer empty-state + report button,
  TitlebarControls donate label, types.ts appError toast messages (no more "Homebrew"/"brew exited").
  ui localStorage key brew-browser→agency-agents.
Verified: svelte-check 0 errors (was 3 brew-CSS warnings → now only the pre-existing tsconfig `node`
note), build green. STILL BREW-NAMED PLUMBING (invisible to user, deferred): types.ts BrewErrorPayload/
isBrewError/BrewStreamEvent type names + dead codes; reportIssue shell-exit semantics (command/exitCode/
stderr) that agency never produces; dead frontend Settings fields (caskIconMode etc.) + backend Settings
struct fields. These are a bounded "frontend brew-plumbing purge" — next.

2026-06-06 — App icon + About rebrand (lead). Built a new app icon from Michael's brain-circuit.svg
(purple→blue→cyan glyph) composited onto a macOS-style rounded square (rsvg-convert glyph → magick
rounded-gradient bg + composite): dark + light 1024 masters in docs/icon/agency-icon-{dark,light}-
1024.png. Generated the full Tauri icon set via `npm run tauri icon docs/icon/agency-icon-dark-1024
.png` (dark = shipped). GOTCHA: incremental `tauri dev` did NOT re-embed the new icns — had to
`touch src-tauri/build.rs` to force tauri-build to re-run, then rebuild + `killall Dock`; then the
Dock showed the new icon. Rebranded AboutModal.svelte: new icon (src/lib/assets/app-icon.png 256px,
drop-shadow), title "Agency Agents", tagline "A native macOS app store for AI agents", repo/license
→ agency-agents-app, credits → built-by-agents + Opus 4.8, dropped Homebrew thanks. Also: Dock-hover
shows "agency-agents-app" (dev binary name) while menu bar shows "Agency Agents" (productName) — a
DEV-ONLY artifact; `tauri build` produces "Agency Agents.app" where both match. Plan persisted to
phase-roadmap.md (v2 sequence). First git commit made. Next: #1 clone-as-source-of-truth.
