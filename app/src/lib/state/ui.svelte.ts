// Transient UI state: the open context menu and the inline rename field.
import type { MenuEntry } from "../menu/types";

export type RenameTarget = { kind: "tab"; tabId: string } | { kind: "session"; sessionId: string; where: "pane" | "sidebar" };

export const ui = $state({
  menu: null as { x: number; y: number; entries: MenuEntry[] } | null,
  renaming: null as RenameTarget | null,
});

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
