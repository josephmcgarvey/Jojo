// How a machine's connection is described in the sidebar and in pane
// overlays (spec 8.3).
import type { ConnectionStatus } from "../backend/ConnectionStatus";
import type { Machine } from "../backend/Machine";

/**
 * The state shown after a machine's label, or null when it is connected.
 * This PC's problems are explained by the banner, so its header shows only
 * a short word.
 */
export function machineStatusText(machine: Pick<Machine, "kind">, status: ConnectionStatus | undefined): string | null {
  switch (status?.state ?? "starting") {
    case "connected":
      return null;
    case "starting":
      return "connecting";
    case "reconnecting":
      return "reconnecting";
    case "auth_failed":
      return "auth failed";
    case "unsupported":
      return "cannot keep sessions";
    case "version_mismatch":
      return machine.kind === "local" ? "restart required" : "upgrade required";
    case "elevated":
    case "failed":
      return "unavailable";
  }
}

/**
 * Spec 8.3 rule 2: the overlay on the panes of a remote machine that is not
 * connected, or null. This PC's panes show the banner instead.
 */
export function disconnectedOverlay(machine: Pick<Machine, "kind" | "label"> | undefined, status: ConnectionStatus | undefined): string | null {
  if (!machine || machine.kind === "local" || status?.state === "connected") return null;
  if (status?.state === "auth_failed") return `Auth failed on ${machine.label}. Load your key with ssh-add, then Reconnect.`;
  if (status?.state === "unsupported") return status.message;
  return `Disconnected. Sessions keep running on ${machine.label}.`;
}
