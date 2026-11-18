// Transient UI state: the open context menu and the inline rename field.
import type { ImportPreview } from "../backend/ImportPreview";
import type { MenuEntry } from "../menu/types";

export type RenameTarget = { kind: "tab"; tabId: string } | { kind: "session"; sessionId: string; where: "pane" | "sidebar" } | { kind: "project"; projectId: string };

/**
 * The open modal dialog. "snippet-edit" edits the snippet with
 * `snippetId`, or creates one when it is null, starting from `command`
 * (text saved from a terminal selection). "snippet-fill" asks for the
 * values of a snippet's placeholders before inserting it, or running it
 * when `run` is true. "import-config" shows what importing the
 * configuration file at `path` would change, and imports it.
 */
export type DialogTarget =
  | { kind: "add-machine" }
  | { kind: "add-project"; machineId: string }
  | { kind: "settings" }
  | { kind: "switcher" }
  | { kind: "snippets" }
  | { kind: "snippet-edit"; snippetId: string | null; command?: string }
  | { kind: "snippet-fill"; snippetId: string; run: boolean }
  | { kind: "import-config"; path: string; preview: ImportPreview };

export const ui = $state({
  menu: null as { x: number; y: number; entries: MenuEntry[] } | null,
  /**
   * The open rename field, and in `draft` the text typed in it so far
   * (RenameField.svelte), which a rebuilt field restores.
   */
  renaming: null as (RenameTarget & { draft?: string }) | null,
  dialog: null as DialogTarget | null,
});

export function openDialog(target: DialogTarget): void {
  ui.menu = null;
  ui.dialog = target;
}

export function closeDialog(): void {
  ui.dialog = null;
}

export function openMenu(x: number, y: number, entries: MenuEntry[]): void {
  if (entries.length === 0) return;
  ui.menu = { x, y, entries };
}

export function closeMenu(): void {
  ui.menu = null;
}

export function beginRename(target: RenameTarget): void {
  ui.menu = null;
  ui.renaming = target;
}

export function endRename(): void {
  ui.renaming = null;
}
