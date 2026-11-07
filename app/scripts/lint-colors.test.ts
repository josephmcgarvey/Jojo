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

  it("catches uppercase color functions", () => {
    const source = `<style>.a { color: RGB(255,0,0); fill: HSL(1,2%,3%); }</style>`;
    expect(findColorLiterals(source).sort()).toEqual(["hsl(", "rgb("].sort());
  });

  it("catches Svelte style: directives with quoted and expression values", () => {
    const source = `<div style:color="red" style:background-color="#fff"></div>`;
    expect(findColorLiterals(source).sort()).toEqual(["#fff", "red"].sort());
  });

  it("still catches a hex literal immediately followed by non-hex text", () => {
    const source = `<style>.a { background:#000000important; }</style>`;
    expect(findColorLiterals(source)).toEqual(["#000000"]);
  });
});
