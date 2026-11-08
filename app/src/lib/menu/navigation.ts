// Keyboard highlight movement and on-screen placement for ContextMenu.svelte.
import { isEnabled, type MenuEntry } from "./types";

export interface Point {
  x: number;
  y: number;
}

export interface Size {
  w: number;
  h: number;
}

export interface Box {
  x: number;
  y: number;
  w: number;
  h: number;
}

/**
 * The next enabled entry after `current` in direction `delta`, wrapping
 * around. With nothing highlighted (`current` < 0), Down starts at the first
 * enabled entry and Up at the last. Returns -1 when no entry is enabled.
 */
export function moveHighlight(entries: MenuEntry[], current: number, delta: 1 | -1): number {
  const n = entries.length;
  if (n === 0) return -1;
  let index = current < 0 ? (delta === 1 ? -1 : n) : current;
  for (let step = 0; step < n; step++) {
    index = (index + delta + n) % n;
    if (isEnabled(entries[index])) return index;
  }
  return -1;
}

export function firstEnabled(entries: MenuEntry[]): number {
  return moveHighlight(entries, -1, 1);
}

/**
 * The top-left corner of a menu opened at `anchor`. It opens to the right of
 * and below the pointer; it flips left of the pointer when it would cross
 * the right edge, and moves up when it would cross the bottom edge, keeping
 * `margin` pixels from the window edges.
 */
export function placeMenu(anchor: Point, size: Size, viewport: Size, margin = 4): Point {
  const x = anchor.x + size.w + margin <= viewport.w ? anchor.x : Math.max(margin, anchor.x - size.w);
  const y = anchor.y + size.h + margin <= viewport.h ? anchor.y : Math.max(margin, viewport.h - size.h - margin);
  return { x, y };
}

/** A submenu opens to the right of its parent row, or to the left of the parent when there is no room. */
export function placeSubmenu(row: Box, size: Size, viewport: Size, margin = 4): Point {
  const right = row.x + row.w;
  const x = right + size.w + margin <= viewport.w ? right : Math.max(margin, row.x - size.w);
  const y = row.y + size.h + margin <= viewport.h ? row.y : Math.max(margin, viewport.h - size.h - margin);
  return { x, y };
}
