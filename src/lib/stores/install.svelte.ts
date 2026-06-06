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

import type { InstalledAgent, InstallRecord, Tool, ToolInfo } from "$lib/types";

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

  /** Adopt a recognized Foreign install into the ledger. */
  async adopt(slug: string, tool: Tool, projectPath: string | null = null): Promise<void> {
    this.busy = `${slug}:${tool}`;
    try {
      await invoke("adopt_agent", { slug, tool, projectPath });
      await this.reconcile();
    } finally {
      this.busy = null;
    }
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
