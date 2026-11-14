// The snippets library's rules, kept free of app state so they can be
// tested: which snippets a terminal lists, how they are grouped, and the
// text a snippet types.
import type { Snippet } from "../backend/Snippet";
import type { SnippetScope } from "../backend/SnippetScope";

/** The location and project of the terminal snippets would go into. */
export interface SnippetTarget {
  machineId: string;
  projectId: string;
}

/**
 * Whether a snippet is listed for a terminal. A snippet for all locations
 * is always listed; one for a location or a project only for a terminal
 * there. Without a target (no terminal focused) only all-location
 * snippets are listed.
 */
export function appliesTo(scope: SnippetScope, target: SnippetTarget | null): boolean {
  switch (scope.kind) {
    case "all":
      return true;
    case "machine":
      return target !== null && scope.machine_id === target.machineId;
    case "project":
      return target !== null && scope.project_id === target.projectId;
  }
}

/** Case-insensitive match of every word of `query` against the snippet's name, command and folder. */
export function matchesQuery(snippet: Snippet, query: string): boolean {
  const haystack = `${snippet.name}\n${snippet.command}\n${snippet.folder}`.toLowerCase();
  return query
    .toLowerCase()
    .split(/\s+/)
    .filter(Boolean)
    .every((word) => haystack.includes(word));
}

export interface SnippetGroup {
  /** The folder name; empty for snippets without one. */
  folder: string;
  snippets: Snippet[];
}

/**
 * Snippets grouped by folder for the Snippets list: snippets without a
 * folder first, then folders in alphabetical order, each sorted by name.
 * Folder names that differ only in case or surrounding spaces are one
 * folder, named as the snippet in it that sorts first by name spells it
 * (trimmed).
 */
export function groupSnippets(snippets: Snippet[]): SnippetGroup[] {
  const byName = (a: Snippet, b: Snippet) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  const groups = new Map<string, SnippetGroup>();
  for (const snippet of [...snippets].sort(byName)) {
    const key = snippet.folder.trim().toLowerCase();
    const group = groups.get(key) ?? { folder: snippet.folder.trim(), snippets: [] };
    group.snippets.push(snippet);
    groups.set(key, group);
  }
  return [...groups.values()].sort((a, b) => (a.folder === "" ? -1 : b.folder === "" ? 1 : a.folder.localeCompare(b.folder, undefined, { sensitivity: "base" })));
}

/** The distinct folder names in use, for the editor's suggestions. */
export function folderNames(snippets: Snippet[]): string[] {
  return groupSnippets(snippets)
    .map((g) => g.folder)
    .filter(Boolean);
}

/**
 * The text a snippet types into a terminal: Windows line endings become
 * line feeds (xterm.js turns each line feed into the carriage return a
 * shell reads as Enter), and trailing line breaks are removed so that
 * inserting a snippet never runs it. Run sends Enter separately.
 */
export function insertionText(command: string): string {
  return command.replace(/\r\n?/g, "\n").replace(/\n+$/, "");
}

/** A name for a snippet saved from selected terminal text: its first non-empty line, at most 48 characters. */
export function nameFromText(text: string): string {
  const line = text.split(/\r?\n/).find((l) => l.trim() !== "")?.trim() ?? "";
  return line.length > 48 ? `${line.slice(0, 47)}…` : line;
}

/**
 * A placeholder: `{{name}}`, or `{{name:default}}` with a default value.
 * The name starts with a letter or underscore and continues with letters,
 * digits, `_` or `-`; spaces inside the braces are ignored. Any other text
 * in double braces is not a placeholder and is typed as written.
 */
const PLACEHOLDER = /\{\{\s*([A-Za-z_][\w-]*)\s*(?::([^{}]*))?\}\}/g;

export interface Placeholder {
  name: string;
  /** The default written in the command (`{{name:default}}`), or "" when there is none. */
  fallback: string;
}

/**
 * The command's placeholders in order of first appearance, one per name:
 * a name used several times is asked for once and fills every use. The
 * default is the first one written for the name.
 */
export function placeholders(command: string): Placeholder[] {
  const found = new Map<string, Placeholder>();
  for (const match of command.matchAll(PLACEHOLDER)) {
    if (!found.has(match[1])) found.set(match[1], { name: match[1], fallback: (match[2] ?? "").trim() });
  }
  return [...found.values()];
}

/** The command with every placeholder replaced by its value, or by its default when no value is given. */
export function fillPlaceholders(command: string, values: Record<string, string>): string {
  return command.replace(PLACEHOLDER, (_whole, name: string, fallback?: string) => values[name] ?? (fallback ?? "").trim());
}

/**
 * The value a placeholder's field starts with: the value last typed for it
 * (the snippet's last_values), else the default written in the command.
 */
export function startingValue(placeholder: Placeholder, lastValues: Partial<Record<string, string>>): string {
  return lastValues[placeholder.name] ?? placeholder.fallback;
}
