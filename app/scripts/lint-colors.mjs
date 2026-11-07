// Fails when a .svelte file outside src/themes contains a color literal in
// a style block or style attribute (spec section 9.9: components use only
// var(--jojo-*) values).
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";
import { fileURLToPath } from "node:url";

const NAMED = /:\s*(white|black|red|green|blue|yellow|orange|purple|pink|gray|grey|cyan|magenta)\b/gi;
const LITERAL = /#[0-9a-f]{3,8}(?![0-9a-f])|rgba?\(|hsla?\(/gi;
// Svelte style directives (style:prop="value" or style:prop={expr}) are not
// CSS declarations syntactically, but their value is one semantically, so it
// is checked the same way a `property: value` pair from a style block is.
const STYLE_DIRECTIVE = /style:[\w-]+=(?:"([^"]*)"|\{([^}]*)\})/g;

/** @param {string} source @returns {string[]} */
export function findColorLiterals(source) {
  const styles = [
    ...[...source.matchAll(/<style[^>]*>([\s\S]*?)<\/style>/g)].map((m) => m[1]),
    ...[...source.matchAll(/style="([^"]*)"/g)].map((m) => `:${m[1]}`),
    ...[...source.matchAll(STYLE_DIRECTIVE)].map((m) => `:${m[1] ?? m[2]}`),
  ];
  const found = [];
  for (const css of styles) {
    for (const m of css.matchAll(LITERAL)) found.push(m[0].toLowerCase());
    for (const m of css.matchAll(NAMED)) found.push(m[1].toLowerCase());
  }
  return found;
}

function svelteFiles(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return name === "themes" ? [] : svelteFiles(path);
    return name.endsWith(".svelte") ? [path] : [];
  });
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
  const src = join(fileURLToPath(new URL(".", import.meta.url)), "..", "src");
  let failed = false;
  for (const file of svelteFiles(src)) {
    const hits = findColorLiterals(readFileSync(file, "utf8"));
    if (hits.length > 0) {
      failed = true;
      console.error(`${relative(process.cwd(), file).split(sep).join("/")}: ${hits.join(", ")}`);
    }
  }
  if (failed) process.exit(1);
  console.log("No color literals in components.");
}
