import { describe, expect, it } from "vitest";
import { toPhysicalRect } from "./snap";

describe("toPhysicalRect", () => {
  it("keeps CSS pixels at scale 1", () => {
    expect(toPhysicalRect({ left: 1188, top: 0, width: 46, height: 38 }, 1)).toEqual({ x: 1188, y: 0, width: 46, height: 38 });
  });

  it("scales and rounds each edge separately", () => {
    expect(toPhysicalRect({ left: 1188, top: 0, width: 46, height: 38 }, 1.25)).toEqual({ x: 1485, y: 0, width: 58, height: 48 });
  });
});
