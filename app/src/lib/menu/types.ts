// Entries of Jojo's right-click menus (spec 9.4). A disabled entry carries
// the reason, which the menu shows as the row's tooltip.

export interface MenuAction {
  kind: "action";
  label: string;
  shortcut?: string;
  disabledReason?: string;
  run: () => void;
}

export interface MenuSubmenu {
  kind: "submenu";
  label: string;
  disabledReason?: string;
  items: MenuEntry[];
}

export interface MenuSeparator {
  kind: "separator";
}

export type MenuEntry = MenuAction | MenuSubmenu | MenuSeparator;

export const SEPARATOR: MenuSeparator = { kind: "separator" };

/** Separators are never enabled; a submenu is enabled only if one of its items is. */
export function isEnabled(entry: MenuEntry): boolean {
  if (entry.kind === "separator") return false;
  if (entry.disabledReason) return false;
  return entry.kind === "action" || entry.items.some(isEnabled);
}
