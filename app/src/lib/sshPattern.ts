// Wildcard Host patterns from ~/.ssh/config (`Host exe.dev *.exe.xyz`). A
// pattern names no single server, so the Add location dialog asks for the
// full name of one it matches; SSH then applies that Host line's settings.

/**
 * Whether `host` (as typed: `name`, or `user@name`) matches an OpenSSH Host
 * pattern, where `*` matches any run of characters and `?` exactly one.
 * SSH compares the host name without the user part, ignoring case.
 */
export function matchesHostPattern(host: string, pattern: string): boolean {
  const name = host.trim().slice(host.trim().lastIndexOf("@") + 1).toLowerCase();
  const source = pattern
    .toLowerCase()
    .split("")
    .map((c) => (c === "*" ? ".*" : c === "?" ? "." : c.replace(/[.+^${}()|[\]\\]/g, "\\$&")))
    .join("");
  return name !== "" && new RegExp(`^${source}$`).test(name);
}

/** An example name for a pattern's placeholder: `*.exe.xyz` → `name.exe.xyz`, `web-??` → `web-01`. */
export function examplePatternName(pattern: string): string {
  let digit = 0;
  return pattern.replace(/\*+/g, "name").replace(/\?/g, () => String(digit++ % 2 === 0 ? 0 : 1));
}
