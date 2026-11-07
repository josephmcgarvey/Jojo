import { expect, it } from "vitest";
import { paneKey } from "./pane";

it("changes the pane key when a session stops being attachable", () => {
  const before = paneKey({ id: "S1", attachable: true });
  const after = paneKey({ id: "S1", attachable: false });
  expect(before).not.toBe(after);
  expect(paneKey({ id: "S1", attachable: true })).toBe(before);
  expect(paneKey({ id: "S2", attachable: true })).not.toBe(before);
});
