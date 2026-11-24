import { describe, expect, it } from "vitest";
import { examplePatternName, matchesHostPattern } from "./sshPattern";

describe("matchesHostPattern", () => {
  it("matches a full name against a wildcard pattern", () => {
    expect(matchesHostPattern("runcloud-mcp.exe.xyz", "*.exe.xyz")).toBe(true);
    expect(matchesHostPattern("Runcloud-MCP.EXE.xyz", "*.exe.xyz")).toBe(true);
    expect(matchesHostPattern("exedev@hyperlocal.exe.xyz", "*.exe.xyz")).toBe(true);
  });

  it("rejects names the pattern does not cover", () => {
    // `runcloud-mcp@exe.xyz` is user runcloud-mcp on host exe.xyz.
    expect(matchesHostPattern("runcloud-mcp@exe.xyz", "*.exe.xyz")).toBe(false);
    expect(matchesHostPattern("exe.xyz", "*.exe.xyz")).toBe(false);
    expect(matchesHostPattern("aexexxyz", "*.exe.xyz")).toBe(false);
    expect(matchesHostPattern("", "*.exe.xyz")).toBe(false);
  });

  it("treats ? as exactly one character", () => {
    expect(matchesHostPattern("web-07", "web-??")).toBe(true);
    expect(matchesHostPattern("web-7", "web-??")).toBe(false);
  });
});

describe("examplePatternName", () => {
  it("fills the wildcards with a sample name", () => {
    expect(examplePatternName("*.exe.xyz")).toBe("name.exe.xyz");
    expect(examplePatternName("web-??")).toBe("web-01");
  });
});
