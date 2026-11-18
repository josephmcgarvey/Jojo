// Flag for later: sessions the user marked to come back to. layout.json's
// `flagged` list holds `<machine id>:<session id>` keys (session ids are
// unique only within one daemon, like project ids). Kept free of app state
// so the rules can be tested.

/** The `flagged` key of a session on a machine. */
export function flagKey(machineId: string, sessionId: string): string {
  return `${machineId}:${sessionId}`;
}

/** `flagged` with `key` added (`on`) or removed, without duplicates; the order of the other keys is kept. */
export function setFlag(flagged: readonly string[], key: string, on: boolean): string[] {
  const rest = flagged.filter((k) => k !== key);
  return on ? [...rest, key] : rest;
}

/**
 * `flagged` after `machineId` listed its sessions: that machine's keys
 * whose session is gone are dropped, and other machines' keys are kept,
 * so a location that is not connected yet keeps its flags.
 */
export function pruneFlags(flagged: readonly string[], machineId: string, sessionIds: Iterable<string>): string[] {
  const prefix = `${machineId}:`;
  const live = new Set([...sessionIds].map((id) => flagKey(machineId, id)));
  return flagged.filter((key) => !key.startsWith(prefix) || live.has(key));
}
