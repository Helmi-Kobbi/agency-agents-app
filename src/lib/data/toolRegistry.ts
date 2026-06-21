/**
 * Tool registry — the SINGLE source of truth for every supported tool.
 *
 * The definitions live as one JSON file per tool under
 * `src-tauri/data/tools/*.json` (also embedded by the Rust backend). This module
 * eager-globs that directory at build time, so the whole frontend — badges,
 * accents, labels, short names, the installable set — derives from it. There is
 * no index and nothing to register: **adding a tool is adding a JSON file**
 * (plus its `icon` SVG under `assets/tools/` if it has one). Removing one is
 * deleting the file.
 */

export interface ToolMeta {
  /** camelCase id — the wire value used by the backend + install ledger. */
  id: string;
  /** Full display name, e.g. "Claude Code". */
  label: string;
  /** Compact name for dense UIs (the coverage matrix), e.g. "Claude". */
  short: string;
  /** kebab id matching the CLI install scripts. */
  kebab: string;
  /** Badge accent color (hex). */
  accent: string;
  /** Brand-mark filename stem under assets/tools/ (`claudecode` → claudecode.svg),
      or null to fall back to the letter mark. */
  icon: string | null;
  /** Installable today (has a native renderer) vs merely recognized. */
  wired: boolean;
  /** Install-menu position (wired tools); unset sorts after, by label. */
  order?: number;
  scope?: { user: boolean; project: boolean };
  detect?: { dirs: string[]; agentsDir: string | null };
  version?: { bin: string; args: string[] };
  format?: string;
  slugFrom?: string;
  dest?: { user: string[]; project: string[] };
}

// One JSON per tool — no list to maintain. Path is relative to this file.
const defMods = import.meta.glob("../../../src-tauri/data/tools/*.json", {
  eager: true,
  import: "default",
}) as Record<string, ToolMeta>;

// Brand marks (Lobe Icons, MIT) — monochrome SVG keyed by filename stem.
const iconMods = import.meta.glob("../assets/tools/*.svg", {
  eager: true,
  query: "?raw",
  import: "default",
}) as Record<string, string>;

const ICONS: Record<string, string> = {};
for (const [path, svg] of Object.entries(iconMods)) {
  const stem = path.split("/").pop()!.replace(/\.svg$/, "");
  ICONS[stem] = svg;
}

/** All tools — by explicit `order` first (wired install-menu order), then label. */
export const TOOLS: ToolMeta[] = Object.values(defMods).sort(
  (a, b) => (a.order ?? 999) - (b.order ?? 999) || a.label.localeCompare(b.label),
);

const BY_ID = new Map(TOOLS.map((t) => [t.id, t]));

export function toolMeta(id: string): ToolMeta | null {
  return BY_ID.get(id) ?? null;
}

/** Accent color for a tool's badge (neutral grey fallback). */
export function toolAccent(id: string): string {
  return BY_ID.get(id)?.accent ?? "#8A8F98";
}

/** Full display label (falls back to the raw id). */
export function toolLabel(id: string): string {
  return BY_ID.get(id)?.label ?? id;
}

/** Compact display name for dense UIs (falls back to the label). */
export function toolShort(id: string): string {
  return BY_ID.get(id)?.short ?? toolLabel(id);
}

/** Inline brand-mark SVG (monochrome, currentColor) for a tool, or null. */
export function toolIcon(id: string): string | null {
  const name = BY_ID.get(id)?.icon;
  return name ? ICONS[name] ?? null : null;
}

/** Single-character fallback mark — the label's first letter, uppercased. */
export function toolMark(label: string): string {
  return (label.trim()[0] ?? "?").toUpperCase();
}

/** The installable tools (wired = has a native renderer). */
export function wiredTools(): ToolMeta[] {
  return TOOLS.filter((t) => t.wired);
}
