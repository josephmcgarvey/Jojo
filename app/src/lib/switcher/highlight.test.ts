import { describe, expect, it } from "vitest";
import { highlightParts } from "./highlight";

describe("highlightParts", () => {
  it("marks every match, ignoring case", () => {
    expect(highlightParts("Error: connect ECONNREFUSED, error again", "error")).toEqual([
      { text: "Error", match: true },
      { text: ": connect ECONNREFUSED, ", match: false },
      { text: "error", match: true },
      { text: " again", match: false },
    ]);
  });

  it("returns the text unmarked without a match or a query", () => {
    expect(highlightParts("all good", "error")).toEqual([{ text: "all good", match: false }]);
    expect(highlightParts("all good", "  ")).toEqual([{ text: "all good", match: false }]);
  });
});
