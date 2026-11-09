import { describe, expect, it } from "vitest";
import { folderSuggestions, parentDir, splitTypedPath } from "./pathSuggest";

describe("splitTypedPath", () => {
  it("splits at the last slash and lists home for a bare name", () => {
    expect(splitTypedPath("~/pro")).toEqual({ dir: "~/", prefix: "pro" });
    expect(splitTypedPath("/srv/")).toEqual({ dir: "/srv/", prefix: "" });
    expect(splitTypedPath("app")).toEqual({ dir: "~/", prefix: "app" });
    expect(splitTypedPath("~")).toEqual({ dir: "~/", prefix: "" });
  });
});

describe("folderSuggestions", () => {
  const entries = [".cache", "Projects", "projects-old", "src"];

  it("suggests matching folders as full paths ending in a slash", () => {
    expect(folderSuggestions("~/", "pro", entries)).toEqual(["~/Projects/", "~/projects-old/"]);
  });

  it("hides dot folders until a dot is typed", () => {
    expect(folderSuggestions("~/", "", entries)).toEqual(["~/Projects/", "~/projects-old/", "~/src/"]);
    expect(folderSuggestions("~/", ".", entries)).toEqual(["~/.cache/"]);
  });
});

describe("parentDir", () => {
  it("goes up one folder, from home to the root, and stays at the root", () => {
    expect(parentDir("~/src/app/")).toBe("~/src/");
    expect(parentDir("~/src/")).toBe("~/");
    expect(parentDir("/srv/app/")).toBe("/srv/");
    expect(parentDir("/srv/")).toBe("/");
    expect(parentDir("~/")).toBe("/");
    expect(parentDir("/")).toBe("/");
  });
});
