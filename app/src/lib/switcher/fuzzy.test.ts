import { describe, expect, it } from "vitest";
import { fuzzyFilter, fuzzyScore } from "./fuzzy";

describe("fuzzyScore", () => {
  it("matches characters in order, ignoring case", () => {
    expect(fuzzyScore("fxl", "Fix the login bug")).not.toBeNull();
    expect(fuzzyScore("lxf", "Fix the login bug")).toBeNull();
    expect(fuzzyScore("", "anything")).toBe(0);
    // A word start ("api") must not take a character the rest of the query needs.
    expect(fuzzyScore("ar", "jar api · This PC · Idle")).not.toBeNull();
  });

  it("prefers word starts and consecutive characters", () => {
    const wordStarts = fuzzyScore("tl", "terminal · login")!;
    const scattered = fuzzyScore("tl", "battle")!;
    expect(wordStarts).toBeGreaterThan(scattered);
    expect(fuzzyScore("log", "login")!).toBeGreaterThan(fuzzyScore("log", "l-o-g")!);
  });
});

describe("fuzzyFilter", () => {
  it("drops non-matches and ranks the best match first", () => {
    const items = ["settings", "new shell", "new claude session"];
    expect(fuzzyFilter(items, "nsh", (s) => s)).toEqual(["new shell"]);
    expect(fuzzyFilter(items, "ns", (s) => s)[0]).toBe("new shell");
    expect(fuzzyFilter(items, "cl", (s) => s)).toEqual(["new claude session"]);
    expect(fuzzyFilter(items, "", (s) => s)).toEqual(items);
  });
});
