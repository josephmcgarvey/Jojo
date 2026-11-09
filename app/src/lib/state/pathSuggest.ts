// Folder suggestions for a path typed on a remote machine (spec 9.2).

/**
 * Splits a typed path into the folder to list and the name being typed:
 * `~/pro` → `{ dir: "~/", prefix: "pro" }`, `/srv/` → `{ dir: "/srv/", prefix: "" }`.
 * A path without a slash lists the home folder (`~/`).
 */
export function splitTypedPath(path: string): { dir: string; prefix: string } {
  const slash = path.lastIndexOf("/");
  if (slash < 0) return { dir: "~/", prefix: path === "~" ? "" : path };
  return { dir: path.slice(0, slash + 1), prefix: path.slice(slash + 1) };
}

/**
 * The full paths to suggest: the listed folder's subfolders whose names
 * start with `prefix` (ignoring case), each ending in `/` so choosing one
 * lists its own subfolders next. Hidden folders are offered only once a
 * `.` has been typed.
 */
export function folderSuggestions(dir: string, prefix: string, entries: string[], limit = 12): string[] {
  const lower = prefix.toLowerCase();
  return entries
    .filter((name) => name.toLowerCase().startsWith(lower) && (prefix.startsWith(".") || !name.startsWith(".")))
    .slice(0, limit)
    .map((name) => `${dir}${name}/`);
}

/**
 * The folder above `dir` (a listed folder ending in `/`): `~/src/app/` →
 * `~/src/`, `/srv/` → `/`. The home folder `~/` goes to the file system
 * root `/`, because its absolute path is known only to the server; `/`
 * stays `/`.
 */
export function parentDir(dir: string): string {
  const trimmed = dir.replace(/\/+$/, "");
  if (trimmed === "" || trimmed === "~") return "/";
  const slash = trimmed.lastIndexOf("/");
  return slash < 0 ? "~/" : trimmed.slice(0, slash + 1);
}
