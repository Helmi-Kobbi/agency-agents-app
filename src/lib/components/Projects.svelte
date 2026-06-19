<script lang="ts">
  /**
   * Projects — the 4th pillar (Agents / Tools / Teams / Projects). The home for
   * per-project deployments: every folder you've installed agents into shows up
   * here as a row with its roster.
   *
   * • onMount → projects.refresh() (soft-fails to an empty list while the
   *   backend `projects_list` command is wired up).
   * • Header: "<N> project(s)" + "Add project…" — picks a folder, refreshes, and
   *   immediately opens the DeployModal targeted at the new project so you can
   *   install into it right away.
   * • Empty: an EmptyState explaining the concept + an "Add project…" CTA.
   * • Otherwise: a list of project rows (folder icon · label · muted path ·
   *   installedCount). Each row expands to reveal that project's roster
   *   (install.installed filtered to (row.projectPath ?? null) === project.path),
   *   reusing the Teams "Your team" row pattern (name + tool Pill).
   *
   * "Deploy…" decision (v1): DeployModal requires a fixed agent SET. Rather than
   * dump the whole corpus (overwhelming) or invent a picker, we seed the modal
   * with the agents ALREADY in that project. The tri-state then reflects the
   * project's current coverage across project-capable tools and lets the user
   * extend/retract that set per tool — the modal becomes "manage this project's
   * loadout". When a project is empty there's nothing to seed, so we surface a
   * hint pointing at Teams / Divisions (the set-deploy entry points) instead of
   * opening an empty modal.
   */
  import { onMount } from "svelte";
  import EmptyState from "./EmptyState.svelte";
  import Pill from "./Pill.svelte";
  import Button from "./Button.svelte";
  import Modal from "./Modal.svelte";
  import InstallModal from "./InstallModal.svelte";
  import FolderIcon from "@lucide/svelte/icons/folder";
  import FolderPlus from "@lucide/svelte/icons/folder-plus";
  import ChevronDown from "@lucide/svelte/icons/chevron-down";
  import LayersIcon from "@lucide/svelte/icons/layers";
  import Trash2 from "@lucide/svelte/icons/trash-2";

  import { install } from "$lib/stores/install.svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { projects } from "$lib/stores/projects.svelte";
  import { PRESET_TEAMS } from "$lib/data/presetTeams";
  import { resolveCategoryIcon } from "$lib/util/categoryIcon";
  import type { InstalledAgent, ProjectInfo } from "$lib/types";

  onMount(() => {
    projects.refresh();
  });

  // ── Per-project roster: rows we (or anyone) deployed into that exact path. ──
  // Keyed by the project's absolute path; null projectPath = global, excluded.
  const rowsByProject = $derived.by(() => {
    const m = new Map<string, InstalledAgent[]>();
    for (const r of install.installed) {
      if (r.state === "removed") continue; // ledger says installed but file gone
      const p = r.projectPath;
      if (p == null) continue; // global scope lives in Teams/Tools, not here
      const arr = m.get(p);
      if (arr) arr.push(r);
      else m.set(p, [r]);
    }
    for (const arr of m.values()) arr.sort((a, b) => a.name.localeCompare(b.name));
    return m;
  });

  function rosterFor(path: string): InstalledAgent[] {
    return rowsByProject.get(path) ?? [];
  }
  // Distinct agent slugs deployed into a project — the SET we seed Deploy… with.
  function slugsFor(path: string): string[] {
    return [...new Set(rosterFor(path).map((r) => r.slug))];
  }

  // ── Expand / collapse a project's roster (mirrors Teams' division groups). ──
  let expanded = $state<Set<string>>(new Set());
  function toggle(path: string) {
    const next = new Set(expanded);
    if (next.has(path)) next.delete(path);
    else next.add(path);
    expanded = next;
  }

  function basename(path: string): string {
    return path.replace(/\/+$/, "").split("/").pop() || path;
  }

  // ── Two-step deploy: first CHOOSE what to put in the project (a division, a
  // preset, or "manage the current roster"), then the DeployModal tri-states it
  // across that project's tools. Choosing-first is what lets an EMPTY project be
  // populated — seeding the modal with [] would make it inert. ──
  let chooserFor = $state<string | null>(null); // project path, or null = closed

  // Division choices: every division that actually has agents in the corpus.
  const divisionChoices = $derived(
    corpus.tiles.map((c) => ({
      key: `div:${c.slug}`,
      label: c.label,
      icon: resolveCategoryIcon(c.icon),
      color: corpus.colorOf(c.slug),
      slugs: corpus.agents.filter((a) => a.category === c.slug).map((a) => a.slug),
    })),
  );

  // DeployModal target (the project) + the chosen set.
  let deployTarget = $state<string | undefined>(undefined);
  let deploySlugs = $state<string[]>([]);
  let deployTitle = $state("");
  const showDeploy = $derived(deployTarget !== undefined);

  function deploySet(path: string, slugs: string[], what: string) {
    chooserFor = null;
    deployTarget = path;
    deploySlugs = slugs;
    deployTitle = `${what} → ${basename(path)}`;
  }
  function closeDeploy() {
    deployTarget = undefined;
    deploySlugs = [];
  }

  // ── Add a project → register + show the chooser so it can be populated. ──
  let adding = $state(false);
  async function addProject() {
    if (adding) return;
    adding = true;
    try {
      const p = await projects.addViaPicker();
      if (!p) return;
      await projects.refresh();
      chooserFor = p;
    } finally {
      adding = false;
    }
  }
</script>

<section class="pr">
  <header class="pr-head">
    <p class="pr-count">{projects.list.length} project{projects.list.length === 1 ? "" : "s"}</p>
    <div class="pr-actions">
      <button class="btn primary" disabled={adding} onclick={addProject}>
        <FolderPlus size={15} /><span>Add project…</span>
      </button>
    </div>
  </header>

  {#if projects.list.length === 0}
    <div class="scroll">
      <EmptyState title="No projects yet">
        {#snippet icon()}<FolderIcon size={48} />{/snippet}
        Install agents into a project — a folder with <strong>Claude</strong>, <strong>Codex</strong>,
        or another tool — and that project shows up here with its roster. Work wherever you like:
        <code>~/Software/*</code>, <code>~/Clean/*</code>, anywhere.
        {#snippet cta()}
          <Button variant="primary" disabled={adding} onclick={addProject}>
            {#snippet icon()}<FolderPlus size={15} />{/snippet}
            Add project…
          </Button>
        {/snippet}
      </EmptyState>
    </div>
  {:else}
    <ul class="rows">
      {#each projects.list as project (project.path)}
        {@const roster = rosterFor(project.path)}
        {@const isOpen = expanded.has(project.path)}
        {@const canExpand = roster.length > 0}
        <li class="proj">
          <div class="proj-main">
            <button
              class="proj-toggle"
              class:bare={!canExpand}
              onclick={() => canExpand && toggle(project.path)}
              aria-expanded={canExpand ? isOpen : undefined}
              disabled={!canExpand}
            >
              {#if canExpand}
                <ChevronDown size={15} class={isOpen ? "proj-chev open" : "proj-chev"} />
              {:else}
                <span class="proj-chev-spacer"></span>
              {/if}
              <span class="proj-ic"><FolderIcon size={18} /></span>
              <span class="proj-body">
                <span class="proj-label">{project.label}</span>
                <span class="proj-path" title={project.path}>{project.path}</span>
              </span>
            </button>
            <span class="proj-count">{project.installedCount} agent{project.installedCount === 1 ? "" : "s"}</span>
            <Button size="sm" variant="secondary" onclick={() => (chooserFor = project.path)}>Deploy…</Button>
            <button class="proj-del" title="Remove from list" aria-label="Remove project from list" onclick={() => projects.unregister(project.path)}><Trash2 size={14} /></button>
          </div>

          {#if canExpand && isOpen}
            <ul class="roster">
              {#each roster as r (r.slug + r.tool)}
                <li class="r-row">
                  <span class="r-name">{r.name}</span>
                  <Pill tone="neutral">{install.toolLabel(r.tool)}</Pill>
                </li>
              {/each}
            </ul>
          {:else if !canExpand}
            <p class="proj-empty">
              <LayersIcon size={14} />
              No agents here yet — hit <strong>Deploy…</strong> to add a division or team.
            </p>
          {/if}
        </li>
      {/each}
    </ul>
  {/if}
</section>

{#if chooserFor !== null}
  {@const path = chooserFor}
  {@const current = slugsFor(path)}
  <Modal open title="Deploy into {basename(path)}" onClose={() => (chooserFor = null)}>
    <p class="ch-sub">Pick a team or division to deploy into this project — then choose its tools.</p>

    {#if current.length > 0}
      <button class="ch-row" onclick={() => deploySet(path, current, "Current roster")}>
        <span class="ch-ic"><FolderIcon size={16} /></span>
        <span class="ch-body"><span class="ch-label">Current roster</span><span class="ch-meta">{current.length} agent{current.length === 1 ? "" : "s"} already here — add/remove across tools</span></span>
      </button>
    {/if}

    <p class="ch-h">Presets</p>
    {#each PRESET_TEAMS as p (p.slug)}
      {@const PIcon = p.icon}
      <button class="ch-row" onclick={() => deploySet(path, p.agents, p.label)}>
        <span class="ch-ic" style="color:{p.color}"><PIcon size={16} /></span>
        <span class="ch-body"><span class="ch-label">{p.label}</span><span class="ch-meta">{p.description}</span></span>
      </button>
    {/each}

    <p class="ch-h">Divisions</p>
    {#each divisionChoices as d (d.key)}
      {@const DIcon = d.icon}
      <button class="ch-row" onclick={() => deploySet(path, d.slugs, d.label)}>
        <span class="ch-ic" style="color:{d.color}"><DIcon size={16} /></span>
        <span class="ch-body"><span class="ch-label">{d.label}</span><span class="ch-meta">{d.slugs.length} agent{d.slugs.length === 1 ? "" : "s"}</span></span>
      </button>
    {/each}

    {#snippet actions()}
      <Button variant="secondary" onclick={() => (chooserFor = null)}>Cancel</Button>
    {/snippet}
  </Modal>
{/if}

{#if showDeploy && deployTarget !== undefined}
  <InstallModal title={deployTitle} agentSlugs={deploySlugs} onClose={closeDeploy} />
{/if}

<style>
  .pr { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .pr-head {
    flex: none; display: flex; align-items: center; justify-content: space-between; gap: var(--space-3);
    padding: var(--space-3) var(--space-4); border-bottom: 1px solid var(--color-border);
  }
  .pr-count { color: var(--color-text-secondary); font-size: var(--text-body-sm); }
  .pr-actions { display: flex; gap: var(--space-2); }

  .btn {
    display: inline-flex; align-items: center; gap: 6px;
    height: 32px; padding: 0 var(--space-3);
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
    background: transparent; color: var(--color-text-secondary);
    font-size: var(--text-body-sm); cursor: pointer;
  }
  .btn:hover:not(:disabled) { color: var(--color-text-primary); background: var(--color-surface-sunken); }
  .btn:disabled { opacity: 0.5; cursor: default; }
  .btn.primary { background: var(--color-brand); color: var(--color-text-inverse); border-color: transparent; }
  .btn.primary:hover:not(:disabled) { filter: brightness(1.08); background: var(--color-brand); }

  .scroll { flex: 1; min-height: 0; overflow-y: auto; }

  /* ── Project rows ── */
  .rows { flex: 1; min-height: 0; overflow-y: auto; list-style: none; margin: 0; padding: var(--space-3) var(--space-3); display: flex; flex-direction: column; gap: var(--space-2); }
  .proj { border: 1px solid var(--color-border); border-radius: var(--radius-lg); background: var(--color-surface-raised); overflow: hidden; }
  .proj-main { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-2) var(--space-3); }
  .proj-toggle {
    flex: 1; min-width: 0; display: flex; align-items: center; gap: var(--space-2);
    background: transparent; border: none; padding: var(--space-1) 0; cursor: pointer; text-align: left;
  }
  .proj-toggle.bare { cursor: default; }
  :global(.proj-chev) { color: var(--color-text-muted); transition: transform var(--motion-duration-fast, 120ms) ease; transform: rotate(-90deg); flex: none; }
  :global(.proj-chev.open) { transform: rotate(0deg); }
  .proj-chev-spacer { flex: none; width: 15px; }
  .proj-ic { flex: none; display: inline-flex; align-items: center; justify-content: center; width: 32px; height: 32px; border-radius: var(--radius-md); background: var(--color-surface-sunken); color: var(--color-text-secondary); }
  .proj-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .proj-label { font-weight: var(--fw-semibold); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .proj-path { font-size: var(--text-caption); color: var(--color-text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .proj-count { flex: none; font-size: var(--text-body-sm); color: var(--color-text-muted); font-variant-numeric: tabular-nums; white-space: nowrap; }
  .proj-del { flex: none; display: inline-flex; align-items: center; justify-content: center; width: 28px; height: 28px; border-radius: var(--radius-md); color: var(--color-text-muted); cursor: pointer; }
  .proj-del:hover { background: var(--color-surface-sunken); color: var(--color-danger); }

  /* ── Deploy chooser (pick a team/division to put into the project) ── */
  .ch-sub { font-size: var(--text-body-sm); color: var(--color-text-secondary); margin-bottom: var(--space-3); }
  .ch-h { font-size: var(--text-caption); font-weight: var(--fw-semibold); color: var(--color-text-muted); text-transform: uppercase; letter-spacing: 0.04em; margin: var(--space-3) 0 var(--space-1); }
  .ch-row { display: flex; align-items: center; gap: var(--space-3); width: 100%; padding: var(--space-2); border-radius: var(--radius-md); background: transparent; cursor: pointer; text-align: left; }
  .ch-row:hover { background: var(--color-surface-sunken); }
  .ch-ic { flex: none; display: inline-flex; align-items: center; justify-content: center; width: 30px; height: 30px; border-radius: var(--radius-md); background: var(--color-surface-sunken); }
  .ch-body { flex: 1; min-width: 0; display: flex; flex-direction: column; gap: 1px; }
  .ch-label { font-weight: var(--fw-medium); color: var(--color-text-primary); }
  .ch-meta { font-size: var(--text-caption); color: var(--color-text-muted); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .roster { list-style: none; margin: 0; padding: 0 var(--space-3) var(--space-2) calc(15px + var(--space-2) + var(--space-3)); display: flex; flex-direction: column; gap: 1px; }
  .r-row { display: flex; align-items: center; gap: var(--space-3); padding: var(--space-2) var(--space-3); border-radius: var(--radius-md); }
  .r-row:hover { background: var(--color-surface-sunken); }
  .r-name { flex: 1; min-width: 0; font-weight: var(--fw-medium); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .proj-empty {
    display: flex; align-items: center; gap: 6px;
    padding: var(--space-1) var(--space-3) var(--space-3) calc(15px + var(--space-2) + var(--space-3));
    font-size: var(--text-caption); color: var(--color-text-muted);
  }

  code {
    font-family: var(--font-mono, ui-monospace, monospace);
    font-size: 0.92em; background: var(--color-surface-sunken);
    padding: 1px 5px; border-radius: var(--radius-sm); color: var(--color-text-secondary);
  }
</style>
