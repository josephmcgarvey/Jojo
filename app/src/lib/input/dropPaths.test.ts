import { describe, expect, it } from "vitest";
import { dropText, quotePath } from "./dropPaths";

describe("quotePath", () => {
  it("double-quotes Windows paths with spaces or special characters", () => {
    expect(quotePath("C:\\Users\\me\\shot.png", "windows")).toBe("C:\\Users\\me\\shot.png");
    expect(quotePath("C:\\Users\\me\\My Pictures\\shot.png", "windows")).toBe('"C:\\Users\\me\\My Pictures\\shot.png"');
    expect(quotePath("C:\\work\\a&b (1).txt", "windows")).toBe('"C:\\work\\a&b (1).txt"');
  });

  it("single-quotes POSIX paths that are not plain", () => {
    expect(quotePath("/home/u/.jojo/drops/1764/shot.png", "posix")).toBe("/home/u/.jojo/drops/1764/shot.png");
    expect(quotePath("/Users/me/Screen Shot.png", "posix")).toBe("'/Users/me/Screen Shot.png'");
    expect(quotePath("/tmp/it's $HOME", "posix")).toBe("'/tmp/it'\\''s $HOME'");
  });
});

describe("dropText", () => {
  it("joins the quoted paths and ends with a space", () => {
    expect(dropText(["C:\\a.png", "C:\\b c.png"], "windows")).toBe('C:\\a.png "C:\\b c.png" ');
    expect(dropText([], "posix")).toBe("");
  });
});
