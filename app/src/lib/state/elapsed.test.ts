import { expect, it } from "vitest";
import { formatElapsed } from "./elapsed";

it("formats elapsed time like the sidebar mockup", () => {
  expect(formatElapsed(-5)).toBe("now");
  expect(formatElapsed(59_000)).toBe("now");
  expect(formatElapsed(2 * 60_000)).toBe("2m");
  expect(formatElapsed(3 * 3_600_000)).toBe("3h");
  expect(formatElapsed(2 * 86_400_000)).toBe("2d");
});
