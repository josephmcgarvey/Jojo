import { describe, expect, it } from "vitest";
import { focusNeighbor, layoutRects } from "./geometry";
import { pane, split } from "./tree";

describe("layoutRects", () => {
  it("gives a single pane the whole area and no dividers", () => {
    expect(layoutRects(pane("A"), null)).toEqual({ panes: [{ sessionId: "A", x: 0, y: 0, w: 1, h: 1 }], dividers: [] });
  });

  it("places row children left to right with a vertical divider between them", () => {
    const { panes, dividers } = layoutRects(split("row", [pane("A"), pane("B")], [0.6, 0.4]), null);
    expect(panes[0]).toMatchObject({ sessionId: "A", x: 0, y: 0, h: 1 });
    expect(panes[0].w).toBeCloseTo(0.6);
    expect(panes[1].x).toBeCloseTo(0.6);
    expect(panes[1].w).toBeCloseTo(0.4);
    expect(dividers).toHaveLength(1);
    expect(dividers[0]).toMatchObject({ path: [], index: 0, direction: "row", y: 0, length: 1, extent: 1 });
    expect(dividers[0].x).toBeCloseTo(0.6);
  });

  it("nests a column split inside a row split", () => {
    const { panes, dividers } = layoutRects(split("row", [pane("A"), split("column", [pane("B"), pane("C")])]), null);
    const c = panes.find((p) => p.sessionId === "C")!;
    expect([c.x, c.y, c.w, c.h].map((v) => Math.round(v * 100))).toEqual([50, 50, 50, 50]);
    const inner = dividers.find((d) => d.direction === "column")!;
    expect(inner).toMatchObject({ path: [1], index: 0 });
    expect([inner.x, inner.y, inner.length, inner.extent].map((v) => Math.round(v * 100))).toEqual([50, 50, 50, 100]);
  });

  it("shows only the zoomed pane, at full size", () => {
    const root = split("row", [pane("A"), pane("B")]);
    expect(layoutRects(root, "B")).toEqual({ panes: [{ sessionId: "B", x: 0, y: 0, w: 1, h: 1 }], dividers: [] });
  });

  it("ignores a zoomed id that is not in the tree", () => {
    expect(layoutRects(split("row", [pane("A"), pane("B")]), "Z").panes).toHaveLength(2);
  });
});

describe("focusNeighbor", () => {
  const grid = layoutRects(split("column", [split("row", [pane("A"), pane("B")]), split("row", [pane("C"), pane("D")])]), null).panes;

  it("moves to the adjacent pane in each direction", () => {
    expect(focusNeighbor(grid, "A", "right")).toBe("B");
    expect(focusNeighbor(grid, "A", "down")).toBe("C");
    expect(focusNeighbor(grid, "D", "left")).toBe("C");
    expect(focusNeighbor(grid, "D", "up")).toBe("B");
  });

  it("returns null at an edge and for an unknown pane", () => {
    expect(focusNeighbor(grid, "A", "left")).toBeNull();
    expect(focusNeighbor(grid, "A", "up")).toBeNull();
    expect(focusNeighbor(grid, "Z", "left")).toBeNull();
  });

  it("picks the pane with the most overlap, then the first in pane order", () => {
    const stack = layoutRects(split("row", [pane("M"), split("column", [pane("S1"), pane("S2")])], [0.6, 0.4]), null).panes;
    expect(focusNeighbor(stack, "M", "right")).toBe("S1");
    expect(focusNeighbor(stack, "S2", "left")).toBe("M");
    const wide = layoutRects(split("column", [split("row", [pane("A"), pane("B")], [0.3, 0.7]), pane("C")]), null).panes;
    expect(focusNeighbor(wide, "C", "up")).toBe("B");
  });
});
