<script lang="ts">
  /**
   * Persona Discover — the Agency Agents corpus browser. The agent-corpus
   * analogue of brew-browser's `Discover.svelte`, reusing its visual language:
   *   • an 18-category tile grid (Lucide icons from `corpus_categories`)
   *   • a search box filtering the corpus by name/description/vibe
   *   • a dense agent grid (emoji + name + vibe + category chip)
   *   • a slide-over PersonaDetail panel (name, description, vibe, rendered
   *     markdown body, inert Install button)
   *
   * ADDITIVE: this lives alongside the inherited brew `Discover.svelte` so the
   * build stays green while the corpus backend is wired phase by phase. The
   * `corpus` store soft-fails to an empty state, so every branch below renders
   * before any Tauri command exists.
   */
  import { onMount } from "svelte";
  import SearchIcon from "@lucide/svelte/icons/search";
  import XIcon from "@lucide/svelte/icons/x";
  import PlusIcon from "@lucide/svelte/icons/plus";
  import ChevronLeft from "@lucide/svelte/icons/chevron-left";
  import Download from "@lucide/svelte/icons/download";

  import Pill from "./Pill.svelte";
  import Input from "./Input.svelte";
  import LoadingState from "./LoadingState.svelte";
  import EmptyState from "./EmptyState.svelte";
  import ResizeHandle from "./ResizeHandle.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import {
    ui,
    DETAIL_PANE_MIN_WIDTH,
    DETAIL_PANE_DEFAULT_WIDTH,
    clampDetailPaneWidth,
  } from "$lib/stores/ui.svelte";
  import { install, SUPPORTED_TOOLS, type ToolDef } from "$lib/stores/install.svelte";
  import { toast } from "$lib/stores/toast.svelte";
  import { resolveCategoryIcon } from "$lib/util/categoryIcon";
  import { renderMarkdown } from "$lib/util/markdown";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import type { Agent, InstallState } from "$lib/types";

  // Pure reader of install state (reconciled globally in +layout); only
  // corpus.ensureLoaded() runs here (its own guard makes it safe).
  onMount(() => corpus.ensureLoaded());

  // ── Install flow ─────────────────────────────────────────────────
  // Multi-select: check one or more tools (remembered across agents/launches),
  // then install into all of them at once. Project-scoped tools (cursor,
  // opencode) prompt for a folder as they're reached.
  let installMenuOpen = $state(false);
  let installing = $state(false);

  async function installSelected(agent: Agent) {
    const tools = SUPPORTED_TOOLS.filter((t) => install.isSelected(t.id));
    if (tools.length === 0) {
      toast.info("Pick at least one tool to install into.");
      return;
    }
    installing = true;
    let ok = 0;
    try {
      for (const tool of tools) {
        let projectPath: string | null = null;
        if (tool.scope === "project") {
          const picked = await openDialog({ directory: true, title: `Install ${agent.name} into ${tool.label}…` });
          if (!picked || Array.isArray(picked)) continue; // skip this tool if cancelled
          projectPath = picked;
        }
        try {
          await install.install(agent.slug, tool.id, projectPath);
          ok++;
        } catch (e) {
          toast.error(`Install failed: ${tool.label}`, String(e));
        }
      }
    } finally {
      installing = false;
    }
    if (ok > 0) {
      toast.success(`Installed ${agent.name}`, `into ${ok} tool${ok === 1 ? "" : "s"}`);
      installMenuOpen = false;
    }
  }

  // Reconciled-state badge shown in the install menu + footer chips, so both
  // surfaces tell the SAME truth (a flat "installed" hid Foreign/Outdated).
  function stateBadge(s: InstallState): { label: string; tone: string } {
    switch (s) {
      case "current":  return { label: "installed", tone: "done" };
      case "outdated": return { label: "update", tone: "warn" };
      case "modified": return { label: "modified", tone: "warn" };
      case "foreign":  return { label: "untracked", tone: "info" };
      case "removed":  return { label: "missing", tone: "danger" };
    }
  }

  async function doUninstall(agent: Agent, tool: ToolDef, projectPath: string | null) {
    try {
      await install.uninstall(agent.slug, tool.id, projectPath);
      toast.info(`Removed ${agent.name}`, `from ${tool.label}`);
    } catch (e) {
      toast.error(`Uninstall failed`, String(e));
    }
  }

  // ── Local view state ─────────────────────────────────────────────
  // Selected category slug drives the dense grid; null = tile-grid browse mode.
  let selectedCategory = $state<string | null>(null);
  let query = $state("");

  // Slide-over: the agent currently shown in the detail panel (null = closed).
  // `detail` holds the full record (incl. body) once `corpus.get` resolves;
  // `detailStub` is the list-view record shown immediately while the body loads.
  let detailStub = $state<Agent | null>(null);
  let detail = $state<Agent | null>(null);
  let detailLoading = $state(false);

  function fmt(n: number): string {
    return n.toLocaleString();
  }

  // Searching OR a selected category both drop us out of the tile grid into
  // the dense agent list. Empty query + no category = browse the tiles.
  let browsing = $derived(selectedCategory === null && query.trim() === "");

  let results = $derived(corpus.filtered(selectedCategory, query));

  /** Header label for the dense-grid view. */
  let resultsTitle = $derived(
    selectedCategory ? corpus.labelOf(selectedCategory) : "All agents",
  );

  /** Rendered, deterministic HTML for the open agent's markdown body. */
  let detailBodyHtml = $derived(renderMarkdown(detail?.body ?? ""));

  function selectCategory(slug: string) {
    selectedCategory = slug;
    query = "";
  }

  function clearCategory() {
    selectedCategory = null;
  }

  async function openAgent(agent: Agent) {
    detailStub = agent;
    detail = agent.body ? agent : null;
    detailLoading = !agent.body;
    // Fetch the full record (with body) if we don't already have it.
    if (!agent.body) {
      const full = await corpus.get(agent.slug);
      // Guard against a race where the user opened a different agent meanwhile.
      if (detailStub?.slug === agent.slug) {
        detail = full;
        detailLoading = false;
      }
    }
  }

  function closeDetail() {
    detailStub = null;
    detail = null;
    detailLoading = false;
  }

  function onWindowKey(e: KeyboardEvent) {
    if (panelAgent && e.key === "Escape") {
      e.preventDefault();
      closeDetail();
    }
  }

  // The record to display in the panel header: full record if loaded, else the
  // list-view stub so name/vibe/category show instantly.
  let panelAgent = $derived(detail ?? detailStub);

  /** Lucide icon name for the active category chip (falls back to HelpCircle). */
  let activeCategoryIcon = $derived(
    corpus.categories.find((c) => c.slug === selectedCategory)?.icon ?? "HelpCircle",
  );
</script>

<svelte:window onkeydown={onWindowKey} />

<section class="discover">
  <div class="search-bar">
    <Input
      bind:value={query}
      variant="search"
      placeholder="Search agents by name, role, or vibe…"
      ariaLabel="Search agents"
    />

    {#if selectedCategory}
      {@const Icon = resolveCategoryIcon(activeCategoryIcon)}
      <div class="chip-bar" aria-label="Active category filter">
        <button
          class="chip on"
          onclick={clearCategory}
          aria-label={`Remove ${corpus.labelOf(selectedCategory)} filter`}
        >
          <Icon size={12} />
          <span>{corpus.labelOf(selectedCategory)}</span>
          <XIcon size={12} />
        </button>
        <button class="chip-clear" onclick={clearCategory}>Clear</button>
      </div>
    {/if}
  </div>

  <div class="results">
    {#if corpus.loading && corpus.agents.length === 0 && corpus.categories.length === 0}
      <LoadingState rows={5} label="Loading agents…" />
    {:else if corpus.error && corpus.agents.length === 0 && corpus.categories.length === 0}
      <EmptyState
        title="Corpus unavailable"
        body="The agent corpus isn't ready yet. It will appear here once the backend is wired."
      >
        {#snippet icon()}<SearchIcon size={48} />{/snippet}
      </EmptyState>
    {:else if browsing}
      <!-- Default: 18-category tile grid -->
      <div class="cat-intro">
        <p class="text-muted">
          Browse {fmt(corpus.agents.length)} agents by category, or type a query
          above to search.
        </p>
      </div>
      <div class="tile-grid" role="grid" aria-label="Categories">
        {#each corpus.tiles as t (t.slug)}
          {@const Icon = resolveCategoryIcon(t.icon)}
          <button
            class="tile"
            role="gridcell"
            onclick={() => selectCategory(t.slug)}
            aria-label={`${t.label} — ${fmt(t.count)} agents`}
          >
            <span class="tile-icon"><Icon size={24} /></span>
            <span class="tile-label">{t.label}</span>
            <span class="tile-count">{fmt(t.count)} agents</span>
          </button>
        {/each}
      </div>
    {:else}
      <!-- Search / category-filtered dense agent grid -->
      <div class="cat-header">
        {#if selectedCategory}
          <button class="back" onclick={clearCategory} aria-label="Back to categories">
            <ChevronLeft size={16} />
          </button>
        {/if}
        <h2>{resultsTitle}</h2>
        <span class="text-muted">{fmt(results.length)} agents</span>
      </div>
      {#if results.length === 0}
        <EmptyState
          title={query.trim()
            ? `Nothing matches "${query.trim()}".`
            : "No agents in this category."}
          body={query.trim() ? "Try a shorter or different term." : ""}
        >
          {#snippet icon()}<SearchIcon size={48} />{/snippet}
        </EmptyState>
      {:else}
        <div class="agent-grid" aria-label={`Agents in ${resultsTitle}`}>
          {#each results as a (a.slug)}
            {@const selected = panelAgent?.slug === a.slug}
            <button
              class="agent-card"
              class:selected
              aria-current={selected ? "true" : undefined}
              onclick={() => openAgent(a)}
            >
              <span class="agent-emoji" aria-hidden="true">{a.emoji ?? "🧩"}</span>
              <span class="agent-body">
                <span class="agent-name truncate">{a.name}</span>
                {#if a.vibe}
                  <span class="agent-vibe">{a.vibe}</span>
                {/if}
                <span class="agent-cat">
                  <Pill tone="brand">{corpus.labelOf(a.category)}</Pill>
                </span>
              </span>
            </button>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</section>

<!-- ── Slide-over PersonaDetail ──────────────────────────────────────
     Self-contained panel local to this section so it doesn't disturb the
     inherited brew PackageDetail global slide-over. -->
{#if panelAgent}
  <button
    class="scrim"
    aria-label="Close agent detail"
    onclick={closeDetail}
  ></button>
  <aside class="persona-detail" style="width: {ui.detailPaneWidth}px" aria-label={`${panelAgent.name} detail`}>
    <div class="pd-resize">
      <ResizeHandle
        width={ui.detailPaneWidth}
        min={DETAIL_PANE_MIN_WIDTH}
        max={900}
        defaultWidth={DETAIL_PANE_DEFAULT_WIDTH}
        direction="left"
        label="Resize agent detail"
        onChange={(w) => (ui.detailPaneWidth = clampDetailPaneWidth(w))}
        onCommit={(w) => ui.setDetailPaneWidth(w)}
      />
    </div>
    <header class="pd-head">
      <span class="pd-emoji" aria-hidden="true">{panelAgent.emoji ?? "🧩"}</span>
      <div class="pd-titles">
        <h2 class="pd-name">{panelAgent.name}</h2>
        <span class="pd-cat"><Pill tone="brand">{corpus.labelOf(panelAgent.category)}</Pill></span>
      </div>
      <button class="pd-close" onclick={closeDetail} aria-label="Close">
        <XIcon size={16} />
      </button>
    </header>

    <div class="pd-body">
      {#if panelAgent.vibe}
        <p class="pd-vibe">{panelAgent.vibe}</p>
      {/if}
      {#if panelAgent.description}
        <p class="pd-desc">{panelAgent.description}</p>
      {/if}

      <div class="pd-persona">
        {#if detailLoading}
          <LoadingState rows={5} label="Loading persona…" />
        {:else if detailBodyHtml}
          <!-- Markdown rendered by our deterministic, escaping renderer
               (util/markdown.ts) — the sole source of this HTML, so {@html}
               is safe here. -->
          <div class="markdown">{@html detailBodyHtml}</div>
        {:else}
          <p class="text-muted">No persona body available.</p>
        {/if}
      </div>
    </div>

    <footer class="pd-foot">
      {#if panelAgent}
      {@const here = install.forSlug(panelAgent.slug)}
      {#if here.length > 0}
        <div class="pd-installed">
          {#each here as row (row.tool + (row.projectPath ?? ""))}
            {@const td = SUPPORTED_TOOLS.find((t) => t.id === row.tool)}
            {@const foreign = row.state === "foreign"}
            <button
              class="installed-chip"
              class:warn={row.state !== "current"}
              title={foreign
                ? `${row.dest} — untracked (installed outside the app). Click to Track it (no files changed).`
                : `${row.dest}${row.state !== "current" ? ` — ${row.state}` : ""} · click to remove`}
              onclick={() => {
                if (!panelAgent) return;
                if (foreign) {
                  void install.track(row.slug, row.tool, row.projectPath ?? null);
                } else {
                  void doUninstall(
                    panelAgent,
                    td ?? { id: row.tool, label: row.tool, scope: row.scope },
                    row.projectPath ?? null,
                  );
                }
              }}
            >
              <span class="ic">{row.state === "current" ? "✓" : "!"}</span>
              <span class="t">{td?.label ?? row.tool}</span>
              {#if foreign}<PlusIcon size={12} />{:else}<XIcon size={12} />{/if}
            </button>
          {/each}
        </div>
      {/if}

      <div class="pd-install-wrap">
        <button class="install-btn" onclick={() => (installMenuOpen = !installMenuOpen)}>
          <Download size={15} />
          <span>Install into…</span>
          {#if install.selectedTools.length > 0}<span class="sel-count">{install.selectedTools.length}</span>{/if}
        </button>
        {#if installMenuOpen}
          <div class="install-menu" role="group" aria-label="Install into tools">
            {#each SUPPORTED_TOOLS as t (t.id)}
              {@const st = panelAgent ? install.stateFor(panelAgent.slug, t.id) : null}
              <label class="install-opt">
                <input
                  type="checkbox"
                  checked={install.isSelected(t.id)}
                  onchange={() => install.toggleSelected(t.id)}
                />
                <span class="t">{t.label}</span>
                {#if t.scope === "project"}<span class="scope-tag">project</span>{/if}
                {#if st}{@const b = stateBadge(st)}<span class="scope-tag {b.tone}">{b.label}</span>{/if}
              </label>
            {/each}
            <button
              class="install-go"
              disabled={install.selectedTools.length === 0 || installing}
              onclick={() => panelAgent && installSelected(panelAgent)}
            >
              <Download size={14} />
              {installing
                ? "Installing…"
                : `Install into ${install.selectedTools.length} tool${install.selectedTools.length === 1 ? "" : "s"}`}
            </button>
          </div>
        {/if}
      </div>
      {/if}
    </footer>
  </aside>
{/if}

<style>
  .discover { display: flex; flex-direction: column; min-height: 0; height: 100%; }

  .search-bar {
    padding: var(--space-4);
    border-bottom: 1px solid var(--color-border);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .search-bar :global(.wrap) { width: 100%; max-width: 480px; }

  /* ── chip bar ─────────────────────────────────────────── */
  .chip-bar {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
    align-items: center;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 2px var(--space-2);
    height: 22px;
    border-radius: var(--radius-full);
    border: 1px solid var(--color-border);
    background: var(--color-surface-sunken);
    color: var(--color-text-secondary);
    font-size: var(--text-body-sm);
    font-weight: var(--fw-medium);
    line-height: 1;
    cursor: pointer;
    transition: background 0.12s ease, border-color 0.12s ease, color 0.12s ease;
  }
  .chip:hover { color: var(--color-text-primary); }
  .chip.on {
    background: var(--color-brand-subtle);
    border-color: var(--color-brand);
    color: var(--color-text-primary);
  }
  .chip-clear {
    padding: 2px var(--space-2);
    height: 22px;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    font-size: var(--text-body-sm);
    background: transparent;
    cursor: pointer;
  }
  .chip-clear:hover { color: var(--color-text-primary); }

  /* ── results ─────────────────────────────────────────── */
  .results { flex: 1; overflow-y: auto; min-height: 0; }

  .cat-intro {
    padding: var(--space-4) var(--space-4) 0 var(--space-4);
    font-size: var(--text-body-sm);
  }

  /* ── category tile grid ─────────────────────────────────── */
  .tile-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
    gap: var(--space-3);
    padding: var(--space-4);
  }
  .tile {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--space-2);
    padding: var(--space-3);
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    text-align: left;
    color: var(--color-text-primary);
    transition: background 0.12s ease, border-color 0.12s ease, transform 0.12s ease;
    cursor: pointer;
  }
  .tile:hover {
    background: var(--color-surface);
    border-color: var(--color-brand);
    transform: translateY(-1px);
  }
  .tile:focus-visible {
    outline: 2px solid var(--color-brand);
    outline-offset: 2px;
  }
  .tile-icon { color: var(--color-brand); display: inline-flex; }
  .tile-label { font-weight: var(--fw-medium); font-size: var(--text-body); }
  .tile-count { font-size: var(--text-body-sm); color: var(--color-text-secondary); }

  /* ── dense agent grid ───────────────────────────────────── */
  .cat-header {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    padding: var(--space-3) var(--space-4);
    border-bottom: 1px solid var(--color-border);
  }
  .cat-header h2 {
    font-size: var(--text-h3, 1.05rem);
    font-weight: var(--fw-medium);
    margin: 0;
  }
  .back {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 26px;
    height: 26px;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    background: transparent;
    cursor: pointer;
  }
  .back:hover { background: var(--color-surface-sunken); color: var(--color-text-primary); }

  .agent-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
    gap: var(--space-3);
    padding: var(--space-4);
  }
  .agent-card {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-3);
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    text-align: left;
    color: var(--color-text-primary);
    cursor: pointer;
    min-width: 0;
    transition: background 0.12s ease, border-color 0.12s ease, transform 0.12s ease;
  }
  .agent-card:hover {
    background: var(--color-surface);
    border-color: var(--color-brand);
    transform: translateY(-1px);
  }
  .agent-card:focus-visible {
    outline: 2px solid var(--color-brand);
    outline-offset: 2px;
  }
  .agent-card.selected {
    border-color: var(--color-brand);
    background: var(--color-brand-subtle);
  }
  .agent-emoji {
    font-size: 22px;
    line-height: 1.2;
    flex: none;
  }
  .agent-body {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  .agent-name {
    font-weight: var(--fw-semibold);
    font-size: var(--text-body);
  }
  .agent-vibe {
    font-size: var(--text-body-sm);
    color: var(--color-text-secondary);
    line-height: var(--lh-normal, 1.4);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }
  .agent-cat { margin-top: 2px; }

  /* ── slide-over PersonaDetail ───────────────────────────── */
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.28);
    border: 0;
    cursor: default;
    z-index: 40;
    animation: scrim-in var(--motion-duration-fast, 120ms) var(--motion-ease-out, ease);
  }
  @keyframes scrim-in { from { opacity: 0; } to { opacity: 1; } }

  .persona-detail {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    /* width set inline from ui.detailPaneWidth; clamped on resize. */
    max-width: 90vw;
    display: flex;
    flex-direction: column;
    background: var(--color-surface-raised);
    border-left: 1px solid var(--color-border);
    box-shadow: var(--shadow-lg, -8px 0 24px rgba(0, 0, 0, 0.18));
    z-index: 41;
    animation: slide-in var(--motion-duration-base, 180ms) var(--motion-ease-out, ease);
  }
  /* Full-height drag strip on the panel's left edge. */
  .pd-resize {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    display: flex;
    z-index: 2;
  }
  @keyframes slide-in {
    from { transform: translateX(16px); opacity: 0; }
    to   { transform: translateX(0); opacity: 1; }
  }
  @media (prefers-reduced-motion: reduce) {
    .scrim, .persona-detail { animation: none; }
  }

  .pd-head {
    display: flex;
    align-items: flex-start;
    gap: var(--space-3);
    padding: var(--space-4);
    border-bottom: 1px solid var(--color-border);
  }
  .pd-emoji { font-size: 28px; line-height: 1.1; flex: none; }
  .pd-titles { display: flex; flex-direction: column; gap: 6px; min-width: 0; flex: 1; }
  .pd-name {
    margin: 0;
    font-size: var(--text-h2, 1.2rem);
    font-weight: var(--fw-semibold);
    color: var(--color-text-primary);
  }
  .pd-cat { display: inline-flex; }
  .pd-close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
    color: var(--color-text-muted);
    background: transparent;
    cursor: pointer;
    flex: none;
  }
  .pd-close:hover { background: var(--color-surface-sunken); color: var(--color-text-primary); }

  .pd-body {
    flex: 1;
    overflow-y: auto;
    min-height: 0;
    padding: var(--space-4);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .pd-vibe {
    margin: 0;
    font-size: var(--text-body);
    font-style: italic;
    color: var(--color-text-secondary);
  }
  .pd-desc {
    margin: 0;
    font-size: var(--text-body-sm);
    color: var(--color-text-primary);
    line-height: var(--lh-normal, 1.5);
  }
  .pd-persona { margin-top: var(--space-2); }

  /* Rendered-markdown typography. Scoped to .markdown so it only styles the
     persona body our renderer emits. */
  .markdown :global(h1),
  .markdown :global(h2),
  .markdown :global(h3),
  .markdown :global(h4),
  .markdown :global(h5),
  .markdown :global(h6) {
    margin: var(--space-4) 0 var(--space-2);
    font-weight: var(--fw-semibold);
    color: var(--color-text-primary);
    line-height: 1.3;
  }
  .markdown :global(h1) { font-size: var(--text-h3, 1.05rem); }
  .markdown :global(h2) { font-size: var(--text-body); }
  .markdown :global(h3),
  .markdown :global(h4),
  .markdown :global(h5),
  .markdown :global(h6) { font-size: var(--text-body-sm); }
  .markdown :global(p) {
    margin: 0 0 var(--space-3);
    font-size: var(--text-body-sm);
    color: var(--color-text-secondary);
    line-height: var(--lh-relaxed, 1.6);
  }
  .markdown :global(ul),
  .markdown :global(ol) {
    margin: 0 0 var(--space-3) var(--space-4);
    padding: 0;
    font-size: var(--text-body-sm);
    color: var(--color-text-secondary);
    line-height: var(--lh-relaxed, 1.6);
  }
  .markdown :global(li) { margin: 2px 0; list-style: disc; }
  .markdown :global(ol li) { list-style: decimal; }
  .markdown :global(hr) {
    border: 0;
    border-top: 1px solid var(--color-border);
    margin: var(--space-4) 0;
  }
  .markdown :global(code) {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 0.85em;
    background: var(--color-surface-sunken);
    padding: 1px 4px;
    border-radius: var(--radius-sm);
  }
  .markdown :global(pre) {
    background: var(--color-surface-sunken);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    padding: var(--space-3);
    overflow-x: auto;
    margin: 0 0 var(--space-3);
  }
  .markdown :global(pre code) { background: transparent; padding: 0; }
  .markdown :global(a) { color: var(--color-brand); text-decoration: underline; }
  .markdown :global(strong) { color: var(--color-text-primary); font-weight: var(--fw-semibold); }

  .pd-foot {
    flex: none;
    padding: var(--space-3) var(--space-4);
    border-top: 1px solid var(--color-border);
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    align-items: stretch;
  }
  .pd-installed {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }
  .installed-chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 26px;
    padding: 0 var(--space-2);
    border-radius: var(--radius-full);
    border: 1px solid var(--color-border);
    background: var(--color-surface-sunken);
    color: var(--color-text-secondary);
    font-size: var(--text-caption);
    font-weight: var(--fw-medium);
    cursor: pointer;
  }
  .installed-chip:hover { color: var(--color-text-primary); border-color: var(--color-danger); }
  .installed-chip .ic { color: var(--color-success); font-weight: var(--fw-bold); }
  .installed-chip.warn .ic { color: var(--color-warning); }
  .pd-install-wrap { position: relative; display: flex; justify-content: flex-end; }
  .install-btn {
    display: inline-flex;
    align-items: center;
    gap: var(--space-2);
    height: 32px;
    padding: 0 var(--space-4);
    border-radius: var(--radius-md);
    background: var(--color-brand);
    color: var(--color-text-inverse);
    font-size: var(--text-body-sm);
    font-weight: var(--fw-medium);
    cursor: pointer;
  }
  .install-btn:hover { filter: brightness(1.08); }
  .install-menu {
    position: absolute;
    bottom: calc(100% + 6px);
    right: 0;
    min-width: 200px;
    background: var(--color-surface-raised);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-md);
    box-shadow: 0 8px 24px -4px color-mix(in oklch, black 35%, transparent);
    padding: 4px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    z-index: 40;
  }
  .install-opt {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    padding: 7px 10px;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--color-text-primary);
    font-size: var(--text-body-sm);
    text-align: left;
    cursor: pointer;
  }
  .install-opt:hover { background: var(--color-surface-sunken); }
  .install-opt input { cursor: pointer; accent-color: var(--color-brand); flex: none; }
  .install-opt .t { flex: 1; }
  .scope-tag {
    font-size: var(--text-caption);
    color: var(--color-text-muted);
    border: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    padding: 0 5px;
  }
  .scope-tag.done { color: var(--color-success); border-color: var(--color-success); }
  .scope-tag.warn { color: var(--color-warning); border-color: var(--color-warning); }
  .scope-tag.info { color: var(--color-brand); border-color: var(--color-brand); }
  .scope-tag.danger { color: var(--color-danger); border-color: var(--color-danger); }
  .install-go {
    margin-top: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: 32px;
    border-top: 1px solid var(--color-border);
    border-radius: var(--radius-sm);
    background: var(--color-brand);
    color: var(--color-text-inverse);
    font-size: var(--text-body-sm);
    font-weight: var(--fw-medium);
    cursor: pointer;
  }
  .install-go:hover:not(:disabled) { filter: brightness(1.08); }
  .install-go:disabled { opacity: 0.5; cursor: default; }
  .sel-count {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 999px;
    background: var(--color-text-inverse);
    color: var(--color-brand);
    font-size: var(--text-caption);
    font-weight: var(--fw-semibold);
  }
</style>
