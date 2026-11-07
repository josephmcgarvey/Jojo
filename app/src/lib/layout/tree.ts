// Pure operations on a tab's split tree (spec sections 9.3 and 9.4). No
// function mutates its input. A valid tree holds 1 to MAX_PANES panes; every
// split has at least two children and one size per child, and its sizes are
// positive fractions that sum to 1. A "row" split places its children side
// by side, left to right; a "column" split stacks them, top to bottom.
import type { LayoutNode } from "../backend/LayoutNode";
import type { SplitDirection } from "../backend/SplitDirection";

export type { LayoutNode, SplitDirection };
export type SplitNode = Extract<LayoutNode, { type: "split" }>;
export type PaneNode = Extract<LayoutNode, { type: "pane" }>;
export type LayoutPreset = "columns" | "rows" | "grid" | "main_stack";

/** Spec section 9.4, "Limits": a tab holds at most 4 panes. */
export const MAX_PANES = 4;

/** Smallest share of its split that a divider drag leaves a child. */
export const MIN_SHARE = 0.1;

export function pane(sessionId: string): PaneNode {
  return { type: "pane", session_id: sessionId };
}

export function equalSizes(n: number): number[] {
  return Array.from({ length: n }, () => 1 / n);
}

export function split(direction: SplitDirection, children: LayoutNode[], sizes?: number[]): SplitNode {
  return { type: "split", direction, children, sizes: sizes ?? equalSizes(children.length) };
}

export function paneIds(node: LayoutNode): string[] {
  return node.type === "pane" ? [node.session_id] : node.children.flatMap(paneIds);
}

export function countPanes(node: LayoutNode): number {
  return paneIds(node).length;
}

export function containsPane(node: LayoutNode, sessionId: string): boolean {
  return paneIds(node).includes(sessionId);
}

export function isFull(node: LayoutNode): boolean {
  return countPanes(node) >= MAX_PANES;
}

function normalizeSizes(sizes: number[]): number[] {
  const valid = sizes.length > 0 && sizes.every((s) => Number.isFinite(s) && s > 0);
  if (!valid) return equalSizes(sizes.length);
  const total = sizes.reduce((sum, s) => sum + s, 0);
  return sizes.map((s) => s / total);
}

/**
 * Restores the tree invariants. Drops splits that have no children, replaces
 * a split that has one child with that child, merges a child split into its
 * parent when both run in the same direction (the child's sizes are scaled
 * by its share of the parent), and rescales sizes to sum to 1. Sizes that
 * are missing, of the wrong length, or not positive finite numbers become
 * equal shares. Returns null for a tree with no panes.
 */
export function normalize(node: LayoutNode): LayoutNode | null {
  if (node.type === "pane") return node;
  const shares = node.sizes.length === node.children.length ? node.sizes : equalSizes(node.children.length);
  const children: LayoutNode[] = [];
  const sizes: number[] = [];
  node.children.forEach((child, index) => {
    const normalized = normalize(child);
    if (!normalized) return;
    const share = shares[index];
    if (normalized.type === "split" && normalized.direction === node.direction) {
      normalized.children.forEach((grandchild, j) => {
        children.push(grandchild);
        sizes.push(share * normalized.sizes[j]);
      });
    } else {
      children.push(normalized);
      sizes.push(share);
    }
  });
  if (children.length === 0) return null;
  if (children.length === 1) return children[0];
  return { type: "split", direction: node.direction, children, sizes: normalizeSizes(sizes) };
}

/**
 * Keeps the panes for which `keep` returns true and normalizes the result,
 * so a removed pane's share goes to its siblings in proportion to their
 * sizes. Returns null when no pane is kept.
 */
export function filterPanes(node: LayoutNode, keep: (sessionId: string) => boolean): LayoutNode | null {
  if (node.type === "pane") return keep(node.session_id) ? node : null;
  const children: LayoutNode[] = [];
  const sizes: number[] = [];
  node.children.forEach((child, index) => {
    const kept = filterPanes(child, keep);
    if (kept) {
      children.push(kept);
      sizes.push(node.sizes[index]);
    }
  });
  if (children.length === 0) return null;
  return normalize({ ...node, children, sizes });
}

function insertBeside(node: LayoutNode, targetId: string, newId: string, direction: SplitDirection): LayoutNode {
  if (node.type === "pane") {
    return node.session_id === targetId ? split(direction, [node, pane(newId)]) : node;
  }
  const index = node.children.findIndex((child) => child.type === "pane" && child.session_id === targetId);
  if (index >= 0 && node.direction === direction) {
    const half = node.sizes[index] / 2;
    const children = [...node.children];
    children.splice(index + 1, 0, pane(newId));
    const sizes = [...node.sizes];
    sizes.splice(index, 1, half, half);
    return { ...node, children, sizes };
  }
  return { ...node, children: node.children.map((child) => insertBeside(child, targetId, newId, direction)) };
}

/**
 * Adds `newId` next to `targetId`: to its right for "row", below it for
 * "column". Inside a split of the same direction the new pane is inserted
 * after the target and the two share the target's former size; otherwise
 * the target is replaced by a new two-pane split. Returns null when the
 * tree is full, the target is missing, or `newId` is already in the tree.
 */
export function splitPane(root: LayoutNode, targetId: string, newId: string, direction: SplitDirection): LayoutNode | null {
  if (isFull(root) || !containsPane(root, targetId) || containsPane(root, newId)) return null;
  return normalize(insertBeside(root, targetId, newId, direction));
}

export function removePane(root: LayoutNode, sessionId: string): LayoutNode | null {
  return filterPanes(root, (id) => id !== sessionId);
}

function mapPanes(node: LayoutNode, rename: (sessionId: string) => string): LayoutNode {
  if (node.type === "pane") return pane(rename(node.session_id));
  return { ...node, children: node.children.map((child) => mapPanes(child, rename)) };
}

/** Exchanges the positions of panes `a` and `b`; unchanged if either is missing. */
export function swapPanes(root: LayoutNode, a: string, b: string): LayoutNode {
  if (a === b || !containsPane(root, a) || !containsPane(root, b)) return root;
  return mapPanes(root, (id) => (id === a ? b : id === b ? a : id));
}

/** Puts `newId` where `oldId` was; unchanged if `oldId` is missing or `newId` is present. */
export function replacePane(root: LayoutNode, oldId: string, newId: string): LayoutNode {
  if (!containsPane(root, oldId) || containsPane(root, newId)) return root;
  return mapPanes(root, (id) => (id === oldId ? newId : id));
}

export function equalize(node: LayoutNode): LayoutNode {
  if (node.type === "pane") return node;
  return { ...node, children: node.children.map(equalize), sizes: equalSizes(node.children.length) };
}

function updateAt(node: LayoutNode, path: number[], update: (node: LayoutNode) => LayoutNode): LayoutNode {
  if (path.length === 0) return update(node);
  if (node.type !== "split" || path[0] < 0 || path[0] >= node.children.length) return node;
  const [head, ...rest] = path;
  const children = node.children.map((child, index) => (index === head ? updateAt(child, rest, update) : child));
  return { ...node, children };
}

/**
 * Moves the boundary between child `index` and child `index + 1` of the
 * split at `path` (child indexes from the root) by `delta`, a fraction of
 * that split's extent; positive moves right or down. Both children keep at
 * least MIN_SHARE. Unchanged when the path does not reach a split or the
 * index does not name a boundary.
 */
export function resizeSplit(root: LayoutNode, path: number[], index: number, delta: number): LayoutNode {
  return updateAt(root, path, (node) => {
    if (node.type !== "split" || index < 0 || index + 1 >= node.children.length) return node;
    const first = node.sizes[index];
    const second = node.sizes[index + 1];
    const lowest = MIN_SHARE - first;
    const highest = second - MIN_SHARE;
    if (lowest > highest) return node;
    const moved = Math.min(highest, Math.max(lowest, delta));
    const sizes = [...node.sizes];
    sizes[index] = first + moved;
    sizes[index + 1] = second - moved;
    return { ...node, sizes };
  });
}

/**
 * Rearranges every pane of a tab (spec 9.4, Layout ▸), keeping the panes'
 * current order. "grid" is 2 columns × 2 rows (with 3 panes the third spans
 * the bottom row; with 2 it is two columns). "main_stack" puts the focused
 * pane on the left at 60% width and stacks the others on the right.
 */
export function applyPreset(root: LayoutNode, preset: LayoutPreset, focusedId: string): LayoutNode {
  const ids = paneIds(root);
  if (ids.length < 2) return root;
  const leaves = ids.map(pane);
  switch (preset) {
    case "columns":
      return split("row", leaves);
    case "rows":
      return split("column", leaves);
    case "grid": {
      if (leaves.length === 2) return split("row", leaves);
      const bottom = leaves.length === 3 ? leaves[2] : split("row", leaves.slice(2, 4));
      return split("column", [split("row", leaves.slice(0, 2)), bottom]);
    }
    case "main_stack": {
      const main = ids.includes(focusedId) ? focusedId : ids[0];
      const others = ids.filter((id) => id !== main).map(pane);
      const stack = others.length === 1 ? others[0] : split("column", others);
      return split("row", [pane(main), stack], [0.6, 0.4]);
    }
  }
}
