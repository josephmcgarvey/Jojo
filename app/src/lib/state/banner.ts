// The connection banner's text and button (Banner.svelte).
import type { ConnectionStatus } from "../backend/ConnectionStatus";

const LOG_PATH = String.raw`%LOCALAPPDATA%\Jojo\logs\jojod.<date>.log`;

export interface BannerView {
  text: string;
  /** Label of the button, which calls restart_daemon (it doubles as Retry); null for no button. */
  action: string | null;
}

/**
 * Banner for a connection status. Until the first connection has succeeded
 * (`everConnected` false) the app has never had sessions on screen, so the
 * texts say jojod is starting or did not start, rather than "Disconnected
 * … Sessions keep running" (Plan 2b carry-over 3.3 item 7).
 */
export function connectionBanner(status: ConnectionStatus, everConnected: boolean): BannerView | null {
  switch (status.state) {
    case "connected":
      return null;
    case "starting":
      return { text: "Starting jojod…", action: null };
    case "reconnecting":
      return everConnected
        ? { text: `Disconnected from jojod: ${status.message}. Reconnecting (attempt ${status.attempt}). Sessions keep running.`, action: null }
        : { text: `Starting jojod (attempt ${status.attempt}): ${status.message}.`, action: null };
    case "elevated":
      return {
        text: "Jojo is running as administrator, and jojod will not run elevated. Close Jojo and start it without administrator rights.",
        action: null,
      };
    case "version_mismatch":
      return { text: `${status.message}.`, action: "Restart jojod (ends all running sessions)" };
    case "failed":
      return everConnected
        ? { text: `Cannot reach jojod: ${status.message}. Logs are in ${LOG_PATH}.`, action: "Retry" }
        : { text: `jojod did not start: ${status.message}. Logs are in ${LOG_PATH}.`, action: "Retry" };
  }
}
