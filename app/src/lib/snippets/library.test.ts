import { describe, expect, it } from "vitest";
import type { Snippet } from "../backend/Snippet";
import { appliesTo, fillPlaceholders, folderNames, groupSnippets, insertionText, matchesQuery, nameFromText, placeholders, startingValue } from "./library";

function snippet(over: Partial<Snippet> = {}): Snippet {
  return { id: "S", name: "Status", command: "git status", folder: "", scope: { kind: "all" }, last_values: {}, ...over };
}

describe("snippets library", () => {
  it("lists all-location snippets everywhere and scoped ones only for their location or project", () => {
    const target = { machineId: "M1", projectId: "P1" };
    expect(appliesTo({ kind: "all" }, target)).toBe(true);
    expect(appliesTo({ kind: "all" }, null)).toBe(true);
    expect(appliesTo({ kind: "machine", machine_id: "M1" }, target)).toBe(true);
    expect(appliesTo({ kind: "machine", machine_id: "M2" }, target)).toBe(false);
    expect(appliesTo({ kind: "machine", machine_id: "M1" }, null)).toBe(false);
    expect(appliesTo({ kind: "project", project_id: "P1" }, target)).toBe(true);
    expect(appliesTo({ kind: "project", project_id: "P2" }, target)).toBe(false);
  });

  it("matches every word of the query against name, command and folder, ignoring case", () => {
    const s = snippet({ name: "Deploy staging", command: "php artisan deploy --env=staging", folder: "Laravel" });
    expect(matchesQuery(s, "")).toBe(true);
    expect(matchesQuery(s, "ARTISAN laravel")).toBe(true);
    expect(matchesQuery(s, "deploy production")).toBe(false);
  });

  it("groups by folder, unfiled first, folders and names in alphabetical order", () => {
    const groups = groupSnippets([
      snippet({ id: "1", name: "push", folder: "Git" }),
      snippet({ id: "2", name: "Logs", folder: "" }),
      snippet({ id: "3", name: "Build", folder: "Deploy" }),
      snippet({ id: "4", name: "add", folder: " git " }),
    ]);
    expect(groups.map((g) => g.folder)).toEqual(["", "Deploy", "git"]);
    expect(groups[2].snippets.map((s) => s.name)).toEqual(["add", "push"]);
    expect(folderNames([snippet({ folder: "Git" }), snippet({ folder: "" })])).toEqual(["Git"]);
  });

  it("types multi-line commands with line feeds and never a trailing Enter", () => {
    expect(insertionText("git add -A\r\ngit commit\r\n\r\n")).toBe("git add -A\ngit commit");
    expect(insertionText("ls\n")).toBe("ls");
  });

  it("names a snippet saved from a selection after its first non-empty line", () => {
    expect(nameFromText("\n  docker compose up -d\nmore")).toBe("docker compose up -d");
    expect(nameFromText("x".repeat(60))).toBe(`${"x".repeat(47)}…`);
  });

  it("finds placeholders once each, in order, with their defaults", () => {
    expect(placeholders("git checkout {{branch}} && git push {{ remote : origin }} {{branch}}")).toEqual([
      { name: "branch", fallback: "" },
      { name: "remote", fallback: "origin" },
    ]);
    expect(placeholders("echo {{1x}} {{}} {{ two words }} {not} ${{HOME}}")).toEqual([{ name: "HOME", fallback: "" }]);
    expect(placeholders("ls -la")).toEqual([]);
  });

  it("fills every use of a placeholder with its value, else its default", () => {
    const command = "git checkout {{branch}} && git push {{remote:origin}} {{branch}}";
    expect(fillPlaceholders(command, { branch: "main" })).toBe("git checkout main && git push origin main");
    expect(fillPlaceholders(command, { branch: "dev", remote: "upstream" })).toBe("git checkout dev && git push upstream dev");
    expect(fillPlaceholders("echo {{ two words }}", {})).toBe("echo {{ two words }}");
  });

  it("starts a field with the last value used, else the default", () => {
    expect(startingValue({ name: "remote", fallback: "origin" }, { remote: "upstream" })).toBe("upstream");
    expect(startingValue({ name: "remote", fallback: "origin" }, {})).toBe("origin");
  });
});
