import { describe, expect, it } from "vitest";
import { shellChoices, shellProgram } from "./shells";

const detected = [
  { label: "PowerShell 7", program: "pwsh" },
  { label: "Windows PowerShell", program: "powershell" },
  { label: "Command Prompt", program: "cmd" },
  { label: "Git Bash", program: "C:\\Program Files\\Git\\bin\\bash.exe" },
];

describe("shellChoices", () => {
  it("lists the default shell first and marks it", () => {
    expect(shellChoices(detected, "cmd").map((c) => c.label)).toEqual([
      "Command Prompt (default)",
      "PowerShell 7",
      "Windows PowerShell",
      "Git Bash",
    ]);
  });

  it("matches the setting regardless of case and an .exe suffix", () => {
    expect(shellChoices(detected, "PWSH.exe")[0]).toEqual({ label: "PowerShell 7 (default)", program: "pwsh", isDefault: true });
  });

  it("keeps detection order and marks nothing when the default was not detected", () => {
    const choices = shellChoices(detected.slice(1), "pwsh");
    expect(choices.map((c) => c.program)).toEqual(["powershell", "cmd", "C:\\Program Files\\Git\\bin\\bash.exe"]);
    expect(choices.some((c) => c.isDefault)).toBe(false);
  });

  it("returns nothing when no shell was detected", () => {
    expect(shellChoices([], "pwsh")).toEqual([]);
  });
});

describe("shellProgram", () => {
  it("uses the default shell on This PC and the login shell on a remote machine", () => {
    expect(shellProgram("shell", undefined, "pwsh", true)).toBe("pwsh");
    expect(shellProgram("shell", undefined, "pwsh", false)).toBeNull();
  });

  it("keeps an explicitly requested program, such as a Restart's own command", () => {
    expect(shellProgram("shell", "/bin/zsh", "pwsh", false)).toBe("/bin/zsh");
    expect(shellProgram("shell", "cmd", "pwsh", true)).toBe("cmd");
  });

  it("sends no program for claude sessions", () => {
    expect(shellProgram("claude", undefined, "pwsh", true)).toBeNull();
  });
});
