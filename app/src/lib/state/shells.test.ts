import { describe, expect, it } from "vitest";
import { shellChoices } from "./shells";

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
