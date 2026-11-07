import { describe, expect, it } from "vitest";
import { findColorLiterals } from "./lint-colors.mjs";

describe("findColorLiterals", () => {
  it("finds hex, rgb and hsl literals and named colors in styles", () => {
    const source = `<div style="color: red"></div><style>.a { background: #fff; border-color: rgba(1,2,3,.5); fill: hsl(1 2% 3%); color: var(--jojo-fg); }</style>`;
    expect(findColorLiterals(source).sort()).toEqual(["#fff", "hsl(", "red", "rgba("].sort());
  });

  it("ignores token references and plain text", () => {
    expect(findColorLiterals(`<p>The red button</p><style>.a { color: var(--jojo-error); }</style>`)).toEqual([]);
  });
});
