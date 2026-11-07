// The "New shell" menu entries (Plan 3 amendment B).
import type { ShellOption } from "../backend/ShellOption";

export interface ShellChoice {
  /** Menu text; the default shell's label ends with " (default)". */
  label: string;
  /** Sent as CreateSession.shell. */
  program: string;
  isDefault: boolean;
}

/** `pwsh`, `pwsh.exe` and `PWSH` name the same program. */
function sameProgram(a: string, b: string): boolean {
  const normalize = (s: string) => s.trim().toLowerCase().replace(/\.exe$/, "");
  return normalize(a) === normalize(b);
}

/**
 * Orders the detected shells for a menu: the one matching the defaultShell
 * setting first, marked "(default)", then the others in detection order. When
 * the setting names a shell that was not detected, no entry is marked and the
 * order is unchanged.
 */
export function shellChoices(shells: ShellOption[], defaultShell: string | null | undefined): ShellChoice[] {
  const choices = shells.map((shell) => {
    const isDefault = !!defaultShell && sameProgram(shell.program, defaultShell);
    return { label: isDefault ? `${shell.label} (default)` : shell.label, program: shell.program, isDefault };
  });
  return [...choices.filter((c) => c.isDefault), ...choices.filter((c) => !c.isDefault)];
}
