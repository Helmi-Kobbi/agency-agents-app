<script lang="ts">
  /**
   * Tools — the AI coding tools Agency Agents can deploy into, whether each is
   * detected on this machine, its scope (user-global vs project), and how many
   * agents are currently installed into it (from the ledger).
   */
  import Pill from "./Pill.svelte";
  import RefreshIcon from "@lucide/svelte/icons/refresh-cw";
  import { install } from "$lib/stores/install.svelte";
  import { toast } from "$lib/stores/toast.svelte";

  // Pure reader — tools/installs are loaded globally in +layout; Rescan
  // re-triggers on user action.
  const tools = $derived(install.tools);
  let scanning = $state(false);

  // Re-detect tools AND re-reconcile installs (e.g. after installing agents
  // outside the app, or installing a new editor). This is the "Tool Update".
  async function rescan() {
    scanning = true;
    try {
      await Promise.all([install.loadTools(), install.reconcile()]);
      toast.success("Rescanned tools", `${install.tools.filter((t) => t.detected).length} detected`);
    } finally {
      scanning = false;
    }
  }
</script>

<section class="tools">
  <header class="t-head">
    <p class="t-sub">{tools.length} supported tools · {tools.filter((t) => t.detected).length} detected here</p>
    <button class="rescan" disabled={scanning} onclick={rescan} title="Re-detect tools + re-scan installs">
      <RefreshIcon size={15} /><span>{scanning ? "Scanning…" : "Rescan"}</span>
    </button>
  </header>

  <div class="grid">
    {#each tools as t (t.tool)}
      <div class="card" class:dim={!t.detected}>
        <div class="c-top">
          <span class="c-name">{t.label}</span>
          {#if t.detected}
            <Pill tone="success">detected</Pill>
          {:else}
            <Pill tone="neutral">not found</Pill>
          {/if}
        </div>
        <div class="c-meta">
          <span class="scope">{t.scope === "user" ? "user-global" : "project-scoped"}</span>
        </div>
        {#if t.userDest}
          <code class="c-dest" title={t.userDest}>{t.userDest.replace(/^.*\/Users\/[^/]+/, "~")}</code>
        {/if}
        <div class="c-foot">
          <span class="count">{t.installedCount}</span>
          <span class="count-label">agent{t.installedCount === 1 ? "" : "s"} installed</span>
        </div>
      </div>
    {/each}
  </div>
</section>

<style>
  .tools { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .t-head {
    flex: none; padding: var(--space-4); border-bottom: 1px solid var(--color-border);
    display: flex; align-items: center; justify-content: space-between; gap: var(--space-3);
  }
  .t-sub { color: var(--color-text-secondary); font-size: var(--text-body-sm); }
  .rescan {
    display: inline-flex; align-items: center; gap: 6px;
    height: 30px; padding: 0 var(--space-3);
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
    background: transparent; color: var(--color-text-secondary);
    font-size: var(--text-body-sm); cursor: pointer;
  }
  .rescan:hover:not(:disabled) { color: var(--color-text-primary); background: var(--color-surface-sunken); }
  .rescan:disabled { opacity: 0.6; cursor: default; }
  .grid {
    overflow-y: auto;
    padding: var(--space-4);
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
    gap: var(--space-3);
  }
  .card {
    display: flex; flex-direction: column; gap: var(--space-2);
    padding: var(--space-3);
    border: 1px solid var(--color-border); border-radius: var(--radius-md);
    background: var(--color-surface-raised);
  }
  .card.dim { opacity: 0.6; }
  .c-top { display: flex; align-items: center; justify-content: space-between; gap: var(--space-2); }
  .c-name { font-weight: var(--fw-semibold); color: var(--color-text-primary); }
  .c-meta { font-size: var(--text-caption); color: var(--color-text-muted); }
  .c-dest {
    font-family: var(--font-mono, monospace);
    font-size: var(--text-caption);
    color: var(--color-text-secondary);
    background: var(--color-surface-sunken);
    padding: 2px 6px; border-radius: var(--radius-sm);
    overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  .c-foot { display: flex; align-items: baseline; gap: 6px; margin-top: auto; padding-top: var(--space-2); }
  .count { font-size: var(--text-h3, 20px); font-weight: var(--fw-bold); color: var(--color-text-primary); }
  .count-label { font-size: var(--text-caption); color: var(--color-text-muted); }
</style>
