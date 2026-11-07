// Rectangles for a tab's panes and dividers, as fractions (0 to 1) of the
// tab's pane area, and directional focus movement (spec 9.6, Alt+Arrow).
import { containsPane, type LayoutNode, type SplitDirection } from "./tree";

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface PaneRect extends Rect {
  sessionId: string;
}

/**
 * The boundary between child `index` and child `index + 1` of the split at
 * `path`. For a "row" split it is a vertical line at `x`, from `y` to
 * `y + length`; for a "column" split it is a horizontal line at `y`, from
 * `x` to `x + length`. `extent` is the split's own width ("row") or height
 * ("column") as a fraction of the pane area; dividing a pointer movement in
 * pixels by `extent × area size in pixels` gives the `resizeSplit` delta.
 */
export interface DividerRect {
  path: number[];
  index: number;
  direction: SplitDirection;
  x: number;
  y: number;
  length: number;
  extent: number;
}

export interface TabRects {
  panes: PaneRect[];
  dividers: DividerRect[];
}

export type FocusDirection = "left" | "right" | "up" | "down";

/**
 * Lays out the tree in the unit square. A zoomed pane that is in the tree
 * is returned alone at full size with no dividers; the other panes are not
 * returned, so the UI unmounts them and they detach (spec 5.5).
 */
export function layoutRects(root: LayoutNode, zoomed: string | null): TabRects {
  if (zoomed && containsPane(root, zoomed)) {
    return { panes: [{ sessionId: zoomed, x: 0, y: 0, w: 1, h: 1 }], dividers: [] };
  }
  const panes: PaneRect[] = [];
  const dividers: DividerRect[] = [];
  const walk = (node: LayoutNode, r: Rect, path: number[]): void => {
    if (node.type === "pane") {
      panes.push({ sessionId: node.session_id, ...r });
      return;
    }
    let offset = 0;
    node.children.forEach((child, index) => {
      const share = node.sizes[index];
      const childRect =
        node.direction === "row"
          ? { x: r.x + offset * r.w, y: r.y, w: share * r.w, h: r.h }
          : { x: r.x, y: r.y + offset * r.h, w: r.w, h: share * r.h };
      walk(child, childRect, [...path, index]);
      offset += share;
      if (index < node.children.length - 1) {
        dividers.push(
          node.direction === "row"
            ? { path, index, direction: "row", x: r.x + offset * r.w, y: r.y, length: r.h, extent: r.w }
            : { path, index, direction: "column", x: r.x, y: r.y + offset * r.h, length: r.w, extent: r.h },
        );
      }
    });
  };
  walk(root, { x: 0, y: 0, w: 1, h: 1 }, []);
  return { panes, dividers };
}

const EPSILON = 1e-6;

/**
 * The pane to focus when moving from `fromId` in `direction`. Candidates lie
 * entirely on that side of the current pane and overlap it on the other
 * axis. The nearest candidate wins; among equally near ones, the one with
 * the largest overlap; among those, the first in `panes` order (depth-first
 * tree order). Returns null when there is no candidate or `fromId` is not
 * in `panes`.
 */
export function focusNeighbor(panes: PaneRect[], fromId: string, direction: FocusDirection): string | null {
  const from = panes.find((p) => p.sessionId === fromId);
  if (!from) return null;
  let best: { id: string; distance: number; overlap: number } | null = null;
  for (const p of panes) {
    if (p.sessionId === fromId) continue;
    const horizontal = direction === "left" || direction === "right";
    const distance =
      direction === "left"
        ? from.x - (p.x + p.w)
        : direction === "right"
          ? p.x - (from.x + from.w)
          : direction === "up"
            ? from.y - (p.y + p.h)
            : p.y - (from.y + from.h);
    const overlap = horizontal
      ? Math.min(from.y + from.h, p.y + p.h) - Math.max(from.y, p.y)
      : Math.min(from.x + from.w, p.x + p.w) - Math.max(from.x, p.x);
    if (distance < -EPSILON || overlap <= EPSILON) continue;
    const nearer = best === null || distance < best.distance - EPSILON;
    const asNearWithMoreOverlap =
      best !== null && Math.abs(distance - best.distance) <= EPSILON && overlap > best.overlap + EPSILON;
    if (nearer || asNearWithMoreOverlap) best = { id: p.sessionId, distance, overlap };
  }
  return best?.id ?? null;
}
