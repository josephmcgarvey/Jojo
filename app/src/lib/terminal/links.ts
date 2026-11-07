// Opens links clicked in a terminal (Plan 2b carry-over 3.1 items 2 and 3).
import { invoke } from "@tauri-apps/api/core";
import { errorText } from "../errors";
import { askConfirm } from "../state/dialog.svelte";
import { showNotice } from "../state/notices.svelte";

/**
 * Opens `url` in the default browser through the open_url command, which
 * accepts only http and https URLs with a host (app/src-tauri/src/links.rs).
 * With `confirmTarget`, the URL is shown in a confirmation dialog first;
 * OSC 8 hyperlinks use it because their visible text is chosen by the
 * program and can differ from the URL. A rejected URL or a failed
 * ShellExecuteW is shown in the pane's header strip.
 */
export async function openLink(sessionId: string, url: string, confirmTarget: boolean): Promise<void> {
  if (confirmTarget) {
    const confirmed = await askConfirm({
      title: "Open link?",
      message: "Link text in a terminal can differ from where the link goes. This link opens:",
      details: [url],
      confirmLabel: "Open link",
    });
    if (!confirmed) return;
  }
  try {
    await invoke("open_url", { url });
  } catch (err) {
    showNotice(sessionId, `Link not opened: ${errorText(err)}`, "error");
  }
}
