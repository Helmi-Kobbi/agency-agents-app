/**
 * Install store — drives the Phase 2 install/reconcile backend
 * (install_agent / uninstall_agent / installs_reconcile / tools_list).
 *
 * Singleton: import `install` everywhere. `reconcile()` refreshes the
 * cross-tool installed view (the 5-state Library model); `install()` /
 * `uninstall()` mutate then re-reconcile so the UI reflects truth.
 *
 * Backend-not-ready posture (matches corpus store): every invoke is wrapped
 * so a missing command degrades to empty state rather than throwing.
 */
import { invoke } from "@tauri-apps/api/core";

import type { AgentDiff, InstalledAgent, InstallRecord, InstallState, Tool, ToolInfo } from "$lib/types";

/** The tools Phase 2 can install to, with display + scope. Mirrors the Rust
    `SUPPORTED` set in `install/mod.rs`. Order = install-menu order. */
export interface ToolDef {
  id: Tool;
  label: string;
  scope: "user" | "project";
}

// Module-level in-flight guard (NOT a class #private field — those can trip up
// Svelte 5's class-$state transform). Coalesces the many on-mount reconcile()
// callers into one heavy scan.
let reconcileInflight: Promise<void> | null = null;

/** Persisted "Install into…" tool selection — remembered across agents/launches. */
const INSTALL_SELECTION_KEY = "agency-agents:install-selection";

export const SUPPORTED_TOOLS: ToolDef[] = [
  { id: "claudeCode", label: "Claude Code", scope: "user" },
  { id: "codex", label: "Codex", scope: "user" },
  { id: "geminiCli", label: "Gemini CLI", scope: "user" },
  { id: "copilot", label: "Copilot", scope: "user" },
  { id: "qwen", label: "Qwen", scope: "user" },
  { id: "cursor", label: "Cursor", scope: "project" },
  { id: "opencode", label: "opencode", scope: "project" },
];

class InstallStore {
  /** Reconciled cross-tool installs (the Library model). */
  installed: InstalledAgent[] = $state([]);
  /** Detected tools + counts (the Tools section). */
  tools: ToolInfo[] = $state([]);
  /** `${slug}:${tool}` currently mid-install/uninstall (for spinners). */
  busy: string | null = $state(null);
  /** True while a reconcile is in flight (drives loading states). */
  reconciling: boolean = $state(false);
  /** True once the first reconcile has completed (so we can tell "empty"
      apart from "not scanned yet"). */
  reconciled: boolean = $state(false);
  /** Tools currently checked in the "Install into…" menu. Persisted so the
      choice is remembered for the next agent and the next launch. */
  selectedTools: Tool[] = $state([]);

  /** Load the remembered tool selection; defaults to Claude Code on first run. */
  loadSelection(): void {
    let parsed: Tool[] = [];
    try {
      const raw = localStorage.getItem(INSTALL_SELECTION_KEY);
      if (raw) {
        const arr = JSON.parse(raw) as unknown;
        if (Array.isArray(arr)) {
          parsed = arr.filter((id): id is Tool => SUPPORTED_TOOLS.some((t) => t.id === id));
        }
      }
    } catch {
      /* ignore */
    }
    this.selectedTools = parsed.length > 0 ? parsed : ["claudeCode"];
  }

  /** Is `tool` checked in the Install-into menu? */
  isSelected(tool: Tool): boolean {
    return this.selectedTools.includes(tool);
  }

  /** Toggle a tool's checked state and persist the selection. */
  toggleSelected(tool: Tool): void {
    this.selectedTools = this.isSelected(tool)
      ? this.selectedTools.filter((t) => t !== tool)
      : [...this.selectedTools, tool];
    try {
      localStorage.setItem(INSTALL_SELECTION_KEY, JSON.stringify(this.selectedTools));
    } catch {
      /* ignore */
    }
  }

  /**
   * Reconcile installs against disk + corpus. Called from many views on mount,
   * so it COALESCES via a module-level in-flight promise: concurrent callers
   * share one scan (the command reads every installed file + sweeps each tool
   * dir). On error we KEEP the previous result rather than blanking the UI.
   */
  async reconcile(): Promise<void> {
    if (reconcileInflight) return reconcileInflight;
    this.reconciling = true;
    reconcileInflight = (async () => {
      try {
        const result = await invoke<InstalledAgent[]>("installs_reconcile");
        this.installed = result;
        this.reconciled = true;
      } catch {
        // keep prior `installed`; just stop the spinner
      } finally {
        this.reconciling = false;
        reconcileInflight = null;
      }
    })();
    return reconcileInflight;
  }

  async loadTools(): Promise<void> {
    try {
      this.tools = await invoke<ToolInfo[]>("tools_list");
    } catch {
      this.tools = [];
    }
  }

  /** All installed rows for an agent across tools/projects. */
  forSlug(slug: string): InstalledAgent[] {
    return this.installed.filter((i) => i.slug === slug);
  }

  /** Whether `slug` is installed in `tool` (matching project for project tools). */
  isInstalled(slug: string, tool: Tool, projectPath: string | null = null): boolean {
    return this.installed.some(
      (i) =>
        i.slug === slug &&
        i.tool === tool &&
        (i.projectPath ?? null) === (projectPath ?? null),
    );
  }

  /** The reconciled state for `slug` in `tool` (current/outdated/modified/
      removed/foreign), or null if there's no install on disk. Lets the UI show
      the SAME truth everywhere instead of a flat "installed". */
  stateFor(slug: string, tool: Tool, projectPath: string | null = null): InstallState | null {
    const row = this.installed.find(
      (i) =>
        i.slug === slug &&
        i.tool === tool &&
        (i.projectPath ?? null) === (projectPath ?? null),
    );
    return row?.state ?? null;
  }

  async install(slug: string, tool: Tool, projectPath: string | null = null): Promise<InstallRecord> {
    this.busy = `${slug}:${tool}`;
    try {
      const rec = await invoke<InstallRecord>("install_agent", { slug, tool, projectPath });
      await this.reconcile();
      void this.loadTools();
      return rec;
    } finally {
      this.busy = null;
    }
  }

  async uninstall(slug: string, tool: Tool, projectPath: string | null = null): Promise<void> {
    this.busy = `${slug}:${tool}`;
    try {
      await invoke("uninstall_agent", { slug, tool, projectPath });
      await this.reconcile();
      void this.loadTools();
    } finally {
      this.busy = null;
    }
  }

  /** Update an Outdated install to the current corpus version. */
  async update(slug: string, tool: Tool, projectPath: string | null = null): Promise<void> {
    this.busy = `${slug}:${tool}`;
    try {
      await invoke("update_agent", { slug, tool, projectPath });
      await this.reconcile();
    } finally {
      this.busy = null;
    }
  }

  /**
   * Track a recognized Foreign install into the ledger NON-DESTRUCTIVELY — the
   * backend records provenance but never writes to the user's file. After this,
   * reconcile shows Current (file already matches the catalog) or Modified (it
   * differs; an explicit Update reconciles it, backing up first).
   */
  async track(slug: string, tool: Tool, projectPath: string | null = null): Promise<void> {
    this.busy = `${slug}:${tool}`;
    try {
      await invoke("track_agent", { slug, tool, projectPath });
      await this.reconcile();
    } finally {
      this.busy = null;
    }
  }

  /** Diff the on-disk file against the canonical render (review before Update). */
  async diff(slug: string, tool: Tool, projectPath: string | null = null): Promise<AgentDiff> {
    return invoke<AgentDiff>("agent_diff", { slug, tool, projectPath });
  }

  /**
   * Run one action across many installs with a SINGLE reconcile at the end
   * (calling install()/update()/etc. in a loop would reconcile per item). Each
   * target is an existing install row, so project tools already know their dest
   * — no folder prompts. Returns {ok, fail} counts.
   */
  async bulk(
    action: "update" | "track" | "uninstall",
    targets: { slug: string; tool: Tool; projectPath: string | null }[],
  ): Promise<{ ok: number; fail: number }> {
    const cmd =
      action === "uninstall" ? "uninstall_agent" : action === "track" ? "track_agent" : "update_agent";
    let ok = 0;
    let fail = 0;
    for (const t of targets) {
      try {
        await invoke(cmd, { slug: t.slug, tool: t.tool, projectPath: t.projectPath });
        ok++;
      } catch {
        fail++;
      }
    }
    await this.reconcile();
    void this.loadTools();
    return { ok, fail };
  }

  /** Label for a tool id (for view-models that only have the wire value). */
  toolLabel(tool: Tool): string {
    return SUPPORTED_TOOLS.find((t) => t.id === tool)?.label ?? tool;
  }

  /** Export the current install set to an Agentfile at `path`. Returns count. */
  async exportLoadout(path: string): Promise<number> {
    return invoke<number>("loadout_export", { path });
  }

  /** Restore an Agentfile from `path`; installs each entry. Returns records. */
  async importLoadout(path: string): Promise<InstallRecord[]> {
    const recs = await invoke<InstallRecord[]>("loadout_import", { path });
    await this.reconcile();
    void this.loadTools();
    return recs;
  }
}

export const install = new InstallStore();
