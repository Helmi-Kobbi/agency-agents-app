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
