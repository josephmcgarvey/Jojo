// How dropped file paths are typed into a terminal.
//
// Windows Terminal and iTerm type a dropped file's path into the terminal,
// quoted when the shell would otherwise split or expand it. Claude Code
// recognizes a typed image path and attaches the image; a shell gets the
// paths as arguments for a command.

/** Characters that make a Windows path need double quotes in PowerShell or cmd. */
const WINDOWS_SPECIAL = /[\s&()[\]{}^=;!'+,`~$@%#]/;

/** A POSIX path that every shell takes as one word without quotes. */
const POSIX_PLAIN = /^[A-Za-z0-9_\-.,/:@%+=]+$/;

/**
 * One path quoted for the shell family that will read it:
 *
 * - `windows`: double quotes when the path holds a space or a character
 *   PowerShell or cmd treats specially (Windows paths cannot contain `"`).
 * - `posix`: unchanged when it is plain, otherwise single quotes, with each
 *   `'` written as `'\''`.
 */
export function quotePath(path: string, style: "windows" | "posix"): string {
  if (style === "windows") return WINDOWS_SPECIAL.test(path) ? `"${path}"` : path;
  return POSIX_PLAIN.test(path) ? path : `'${path.replace(/'/g, "'\\''")}'`;
}

/**
 * The text typed for a drop: every path quoted and separated by a space,
 * with a trailing space so the next typed word does not join the last path.
 */
export function dropText(paths: readonly string[], style: "windows" | "posix"): string {
  return paths.length === 0 ? "" : `${paths.map((p) => quotePath(p, style)).join(" ")} `;
}
