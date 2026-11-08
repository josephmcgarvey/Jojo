// Transient UI state: the open context menu and the inline rename field.
import type { MenuEntry } from "../menu/types";

export type RenameTarget = { kind: "tab"; tabId: string } | { kind: "session"; sessionId: string; where: "pane" | "sidebar" };

/** The open modal dialog: Add machine, or Add project on a remote machine. */
export type DialogTarget = { kind: "add-machine" } | { kind: "add-project"; machineId: string };

export const ui = $state({
  menu: null as { x: number; y: number; entries: MenuEntry[] } | null,
  renaming: null as RenameTarget | null,
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
