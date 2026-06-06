<script lang="ts">
  /**
   * Agency Dashboard — the at-a-glance rollup that replaces brew's Dashboard.
   * Available agents, what's installed/needs-attention across tools, detected
   * tools, and the catalog's category shape.
   */
  import { resolveCategoryIcon } from "$lib/util/categoryIcon";
  import { onMount } from "svelte";
  import { corpus } from "$lib/stores/corpus.svelte";
  import { install } from "$lib/stores/install.svelte";
  import { ui } from "$lib/stores/ui.svelte";

  // Pure reader — install state loaded globally in +layout.
  onMount(() => corpus.ensureLoaded());

  const available = $derived(corpus.agents.length);
  const managed = $derived(install.installed.filter((i) => i.state !== "foreign").length);
  const attention = $derived(
    install.installed.filter((i) => ["outdated", "modified", "removed"].includes(i.state)).length,
  );
  const foreign = $derived(install.installed.filter((i) => i.state === "foreign").length);
  const detected = $derived(install.tools.filter((t) => t.detected));

  // Top categories by agent count, with a relative bar.
  const topCats = $derived(
    [...corpus.categories].sort((a, b) => b.count - a.count).slice(0, 8),
  );
  const maxCat = $derived(Math.max(1, ...topCats.map((c) => c.count)));
</script>

<section class="dash">
  <div class="stats">
    <button class="stat" onclick={() => ui.setSection("personas")}>
      <span class="s-num">{available}</span>
      <span class="s-lbl">agents available</span>
    </button>
    <button class="stat" onclick={() => ui.setSection("library")}>
      <span class="s-num">{managed}</span>
      <span class="s-lbl">installed by you</span>
    </button>
    <button class="stat" class:warn={attention > 0} onclick={() => ui.setSection("library")}>
      <span class="s-num">{attention}</span>
      <span class="s-lbl">need attention</span>
    </button>
    <button class="stat" class:info={foreign > 0} onclick={() => ui.setSection("library")}>
      <span class="s-num">{foreign}</span>
      <span class="s-lbl">found to adopt</span>
    </button>
  </div>

  <div class="cols">
    <div class="card">
      <h3 class="c-title">Catalog by category</h3>
      <ul class="cats">
        {#each topCats as c (c.slug)}
          {@const Icon = resolveCategoryIcon(c.icon)}
          <li class="cat">
            <button class="cat-btn" onclick={() => ui.setSection("personas")}>
              <span class="cat-ic"><Icon size={15} /></span>
              <span class="cat-label">{c.label}</span>
              <span class="cat-bar"><span class="cat-fill" style="width:{(c.count / maxCat) * 100}%"></span></span>
              <span class="cat-count">{c.count}</span>
            </button>
          </li>
        {/each}
      </ul>
    </div>

    <div class="card">
      <h3 class="c-title">Tools on this Mac</h3>
      {#if detected.length === 0}
        <p class="muted">No supported AI tools detected yet.</p>
      {:else}
        <ul class="tools">
          {#each detected as t (t.tool)}
            <li class="tool">
              <button class="tool-btn" onclick={() => ui.setSection("tools")}>
                <span class="tool-dot"></span>
                <span class="tool-name">{t.label}</span>
                <span class="tool-count">{t.installedCount}</span>
              </button>
            </li>
          {/each}
        </ul>
      {/if}
      <button class="link" onclick={() => ui.setSection("tools")}>Manage tools →</button>
    </div>
  </div>
</section>

<style>
  .dash { height: 100%; overflow-y: auto; padding: var(--space-5); display: flex; flex-direction: column; gap: var(--space-5); }
  .stats { display: grid; grid-template-columns: repeat(auto-fit, minmax(160px, 1fr)); gap: var(--space-3); }
  .stat {
    display: flex; flex-direction: column; gap: 4px; align-items: flex-start;
    padding: var(--space-4); border: 1px solid var(--color-border); border-radius: var(--radius-lg);
    background: var(--color-surface-raised); cursor: pointer; text-align: left;
  }
  .stat:hover { border-color: var(--color-brand); }
  .s-num { font-size: 30px; font-weight: var(--fw-bold); color: var(--color-text-primary); line-height: 1; }
  .s-lbl { font-size: var(--text-body-sm); color: var(--color-text-muted); }
  .stat.warn .s-num { color: var(--color-warning); }
  .stat.info .s-num { color: var(--color-info, var(--color-brand)); }

  .cols { display: grid; grid-template-columns: 1fr 1fr; gap: var(--space-4); align-items: start; }
  @media (max-width: 820px) { .cols { grid-template-columns: 1fr; } }
  .card { border: 1px solid var(--color-border); border-radius: var(--radius-lg); background: var(--color-surface-raised); padding: var(--space-4); }
  .c-title { font-size: var(--text-body-sm); font-weight: var(--fw-semibold); color: var(--color-text-secondary); margin-bottom: var(--space-3); text-transform: uppercase; letter-spacing: 0.04em; }
  .muted { color: var(--color-text-muted); font-size: var(--text-body-sm); }

  .cats { display: flex; flex-direction: column; gap: 2px; }
  .cat-btn {
    display: flex; align-items: center; gap: var(--space-2); width: 100%;
    padding: 6px var(--space-2); border-radius: var(--radius-sm);
    background: transparent; cursor: pointer; text-align: left;
  }
  .cat-btn:hover { background: var(--color-surface-sunken); }
  .cat-ic { display: inline-flex; color: var(--color-text-secondary); }
  .cat-label { width: 130px; font-size: var(--text-body-sm); color: var(--color-text-primary); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .cat-bar { flex: 1; height: 6px; background: var(--color-surface-sunken); border-radius: var(--radius-full); overflow: hidden; }
  .cat-fill { display: block; height: 100%; background: var(--color-brand); border-radius: var(--radius-full); }
  .cat-count { width: 28px; text-align: right; font-size: var(--text-caption); color: var(--color-text-muted); }

  .tools { display: flex; flex-direction: column; gap: 1px; margin-bottom: var(--space-3); }
  .tool-btn {
    display: flex; align-items: center; gap: var(--space-2); width: 100%;
    padding: 6px var(--space-2); border-radius: var(--radius-sm);
    background: transparent; cursor: pointer; text-align: left;
  }
  .tool-btn:hover { background: var(--color-surface-sunken); }
  .tool-dot { width: 8px; height: 8px; border-radius: var(--radius-full); background: var(--color-success); flex: none; }
  .tool-name { flex: 1; font-size: var(--text-body-sm); color: var(--color-text-primary); }
  .tool-count { font-size: var(--text-caption); color: var(--color-text-muted); }
  .link { background: transparent; color: var(--color-brand); font-size: var(--text-body-sm); cursor: pointer; padding: 0; }
</style>
