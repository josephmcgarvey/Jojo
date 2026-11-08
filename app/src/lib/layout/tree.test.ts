import { describe, expect, it } from "vitest";
import {
  applyPreset,
  containsPane,
  countPanes,
  equalize,
  filterPanes,
  isFull,
  MAX_PANES,
  MIN_SHARE,
  normalize,
  pane,
  paneIds,
  removePane,
  replacePane,
  resizeSplit,
  split,
  splitPane,
  swapPanes,
  type LayoutNode,
} from "./tree";

const a = pane("A");
const b = pane("B");
const c = pane("C");
const d = pane("D");

/** Rounds every size to 3 decimals so trees with float sizes compare with toEqual. */
function rounded(node: LayoutNode | null): LayoutNode | null {
  if (!node || node.type === "pane") return node;
  return {
    ...node,
    children: node.children.map((child) => rounded(child)!),
    sizes: node.sizes.map((s) => Math.round(s * 1000) / 1000),
  };
}

describe("paneIds", () => {
  it("lists panes depth first, left to right and top to bottom", () => {
    const root = split("row", [a, split("column", [b, c])]);
    expect(paneIds(root)).toEqual(["A", "B", "C"]);
    expect(countPanes(root)).toBe(3);
    expect(containsPane(root, "C")).toBe(true);
    expect(containsPane(root, "Z")).toBe(false);
  });

  it("gives a new split equal sizes", () => {
    expect(split("row", [a, b, c]).sizes.map((s) => Math.round(s * 1000))).toEqual([333, 333, 333]);
  });
});

describe("splitPane", () => {
  it("turns a single pane into a split of two halves", () => {
    expect(splitPane(a, "A", "B", "row")).toEqual({ type: "split", direction: "row", children: [a, b], sizes: [0.5, 0.5] });
  });

  it("inserts after the target in a split of the same direction, halving the target's share", () => {
    const root = split("row", [a, b], [0.6, 0.4]);
    expect(rounded(splitPane(root, "A", "C", "row"))).toEqual(split("row", [a, c, b], [0.3, 0.3, 0.4]));
  });

  it("nests a new split when the direction differs", () => {
    const root = split("row", [a, b]);
    expect(splitPane(root, "B", "C", "column")).toEqual(split("row", [a, split("column", [b, c])]));
  });

  it("refuses a fifth pane, an unknown target and an id already in the tree", () => {
    const full = split("row", [a, b, c, d]);
    expect(MAX_PANES).toBe(4);
    expect(isFull(full)).toBe(true);
    expect(splitPane(full, "A", "E", "row")).toBeNull();
    expect(splitPane(a, "X", "B", "row")).toBeNull();
    expect(splitPane(split("row", [a, b]), "A", "B", "column")).toBeNull();
  });
});

describe("removePane", () => {
  it("returns the remaining pane when one of two is removed", () => {
    expect(removePane(split("row", [a, b]), "A")).toEqual(b);
  });

  it("gives the removed share to the siblings in proportion", () => {
    const root = split("row", [a, b, c], [0.5, 0.25, 0.25]);
    expect(rounded(removePane(root, "A"))).toEqual(split("row", [b, c], [0.5, 0.5]));
  });

  it("returns null when the last pane is removed", () => {
    expect(removePane(a, "A")).toBeNull();
  });

  it("collapses a one-child split and merges it into a parent of the same direction", () => {
    const root = split("row", [a, split("column", [b, split("row", [c, d])])], [0.5, 0.5]);
    expect(rounded(removePane(root, "B"))).toEqual(split("row", [a, c, d], [0.5, 0.25, 0.25]));
  });

  it("leaves the tree unchanged for an unknown id", () => {
    const root = split("row", [a, b]);
    expect(removePane(root, "Z")).toEqual(root);
  });
});

describe("normalize", () => {
  it("replaces invalid or mismatched sizes with equal shares", () => {
    expect(normalize({ type: "split", direction: "row", children: [a, b], sizes: [Number.NaN, 1] })).toEqual(split("row", [a, b]));
    expect(normalize({ type: "split", direction: "row", children: [a, b], sizes: [1] })).toEqual(split("row", [a, b]));
    expect(normalize({ type: "split", direction: "row", children: [a, b], sizes: [-1, 2] })).toEqual(split("row", [a, b]));
  });

  it("rescales sizes to sum to 1", () => {
    expect(normalize({ type: "split", direction: "column", children: [a, b], sizes: [3, 1] })).toEqual(split("column", [a, b], [0.75, 0.25]));
  });

  it("replaces a one-child split with the child and drops an empty split", () => {
    expect(normalize({ type: "split", direction: "row", children: [a], sizes: [1] })).toEqual(a);
    expect(normalize({ type: "split", direction: "row", children: [], sizes: [] })).toBeNull();
  });
});

describe("filterPanes", () => {
  it("keeps the panes the predicate accepts, in order", () => {
    const root = split("row", [a, split("column", [b, c]), d]);
    expect(paneIds(filterPanes(root, (id) => id !== "B" && id !== "D")!)).toEqual(["A", "C"]);
    expect(filterPanes(root, () => false)).toBeNull();
  });
});

describe("swap, replace and equalize", () => {
  it("exchanges two panes' positions", () => {
    const root = split("row", [a, split("column", [b, c])]);
    expect(paneIds(swapPanes(root, "A", "C"))).toEqual(["C", "B", "A"]);
    expect(swapPanes(root, "A", "Z")).toBe(root);
  });

  it("replaces one session id with another in place", () => {
    const root = split("row", [a, b], [0.7, 0.3]);
    expect(replacePane(root, "A", "N")).toEqual(split("row", [pane("N"), b], [0.7, 0.3]));
    expect(replacePane(root, "A", "B")).toBe(root);
  });

  it("resets every split to equal shares", () => {
    const root = split("row", [a, split("column", [b, c], [0.9, 0.1])], [0.8, 0.2]);
    expect(equalize(root)).toEqual(split("row", [a, split("column", [b, c])]));
  });
});

describe("resizeSplit", () => {
  it("moves the boundary after the given child", () => {
    const root = split("row", [a, b]);
    expect(rounded(resizeSplit(root, [], 0, 0.2))).toEqual(split("row", [a, b], [0.7, 0.3]));
  });

  it("resizes a nested split by its path", () => {
    const root = split("row", [a, split("column", [b, c])]);
    expect(rounded(resizeSplit(root, [1], 0, -0.25))).toEqual(split("row", [a, split("column", [b, c], [0.25, 0.75])]));
  });

  it("keeps both sides at or above MIN_SHARE", () => {
    const root = split("row", [a, b]);
    const moved = resizeSplit(root, [], 0, 5) as Extract<LayoutNode, { type: "split" }>;
    expect(moved.sizes[1]).toBeCloseTo(MIN_SHARE);
    expect(moved.sizes[0]).toBeCloseTo(1 - MIN_SHARE);
  });

  it("ignores a path or index that does not name a boundary", () => {
    const root = split("row", [a, b]);
    expect(resizeSplit(root, [0], 0, 0.1)).toEqual(root);
    expect(resizeSplit(root, [], 1, 0.1)).toEqual(root);
  });
});

describe("applyPreset", () => {
  const four = split("row", [a, split("column", [b, c]), d]);

  it("lays out columns and rows in pane order", () => {
    expect(applyPreset(four, "columns", "A")).toEqual(split("row", [a, b, c, d]));
    expect(applyPreset(four, "rows", "A")).toEqual(split("column", [a, b, c, d]));
  });

  it("builds a 2x2 grid, with the third pane full width when there are three", () => {
    expect(applyPreset(four, "grid", "A")).toEqual(split("column", [split("row", [a, b]), split("row", [c, d])]));
    expect(applyPreset(split("row", [a, b, c]), "grid", "A")).toEqual(split("column", [split("row", [a, b]), c]));
    expect(applyPreset(split("column", [a, b]), "grid", "A")).toEqual(split("row", [a, b]));
  });

  it("puts the focused pane at 60% on the left and stacks the others", () => {
    expect(applyPreset(four, "main_stack", "C")).toEqual(split("row", [c, split("column", [a, b, d])], [0.6, 0.4]));
    expect(applyPreset(split("column", [a, b]), "main_stack", "B")).toEqual(split("row", [b, a], [0.6, 0.4]));
  });

  it("leaves a single pane unchanged", () => {
    expect(applyPreset(a, "grid", "A")).toBe(a);
  });
});
