import { expect, it } from "vitest";
import { flagKey, pruneFlags, setFlag } from "./flags";

it("adds and removes a flag without duplicates", () => {
  const key = flagKey("local", "S1");
  expect(key).toBe("local:S1");
  expect(setFlag([], key, true)).toEqual(["local:S1"]);
  expect(setFlag(["local:S1", "M:S2"], key, true)).toEqual(["M:S2", "local:S1"]);
  expect(setFlag(["local:S1", "M:S2"], key, false)).toEqual(["M:S2"]);
  expect(setFlag(["M:S2"], key, false)).toEqual(["M:S2"]);
});

it("drops the flags of a machine's removed sessions and keeps other machines' flags", () => {
  const flagged = ["local:S1", "local:S2", "M:S3"];
  expect(pruneFlags(flagged, "local", ["S1"])).toEqual(["local:S1", "M:S3"]);
  expect(pruneFlags(flagged, "M", [])).toEqual(["local:S1", "local:S2"]);
  // A machine id that is a prefix of another is not confused with it.
  expect(pruneFlags(["M:S3", "M2:S4"], "M", ["S3"])).toEqual(["M:S3", "M2:S4"]);
});
