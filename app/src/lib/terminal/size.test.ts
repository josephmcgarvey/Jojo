import { expect, it } from "vitest";
import { clampSize } from "./size";

it("keeps sizes within what jojod accepts", () => {
  expect(clampSize(0, 0)).toEqual({ cols: 2, rows: 1 });
  expect(clampSize(Number.NaN, Number.NaN)).toEqual({ cols: 2, rows: 1 });
  expect(clampSize(120.7, 30.2)).toEqual({ cols: 120, rows: 30 });
  expect(clampSize(5000, 5000)).toEqual({ cols: 1000, rows: 1000 });
});
