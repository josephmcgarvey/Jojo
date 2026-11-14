import { describe, expect, it } from "vitest";
import { foldedStates, setFolded } from "./folding";

describe("sidebar folding", () => {
  it("folds and unfolds ids without duplicates, keeping the others", () => {
    expect(setFolded([], ["local"], true)).toEqual(["local"]);
    expect(setFolded(["local", "P1"], ["P1"], true)).toEqual(["local", "P1"]);
    expect(setFolded(["local", "P1", "P2"], ["P1"], false)).toEqual(["local", "P2"]);
    expect(setFolded(["P1"], ["P2", "P3", "P2"], true)).toEqual(["P1", "P2", "P3"]);
    expect(setFolded(["P1", "P2", "P3"], ["P2", "P3"], false)).toEqual(["P1"]);
  });

  it("summarizes a folded group as the colored states of its sessions, in order", () => {
    const states = foldedStates([{ state: "idle" }, { state: "working" }, { state: "running" }, { state: "needs_you" }, { state: "exited" }, { state: "done" }, { state: "error" }, { state: "starting" }]);
    expect(states).toEqual(["working", "needs_you", "done", "error"]);
  });
});
