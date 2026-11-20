// The sidebar's width, which the user sets by dragging its right edge
// (App.svelte) and which is saved as the sidebarWidth setting.

/** The width of a new install, and of a double-click on the edge. */
export const DEFAULT_SIDEBAR_WIDTH = 268;
/** Narrower, and session names and the Needs you queue no longer fit. */
export const MIN_SIDEBAR_WIDTH = 200;
/** Wider, and the panes lose more room than the sidebar gains. */
export const MAX_SIDEBAR_WIDTH = 480;
/** Dragging the edge to a width below this hides the sidebar on release. */
export const HIDE_SIDEBAR_BELOW = 120;
/** How far one press of Left or Right on the focused edge moves it. */
export const SIDEBAR_KEY_STEP = 16;

/** `width` rounded and kept within MIN_SIDEBAR_WIDTH to MAX_SIDEBAR_WIDTH; the default for a value that is not a number. */
/** Whether a drag of the sidebar's edge that ends at `width` (before clamping) hides the sidebar. */
export function dragHidesSidebar(width: number): boolean {
  return width < HIDE_SIDEBAR_BELOW;
}

export function clampSidebarWidth(width: number | null | undefined): number {
  if (width === null || width === undefined || !Number.isFinite(width)) return DEFAULT_SIDEBAR_WIDTH;
  return Math.round(Math.min(MAX_SIDEBAR_WIDTH, Math.max(MIN_SIDEBAR_WIDTH, width)));
}
