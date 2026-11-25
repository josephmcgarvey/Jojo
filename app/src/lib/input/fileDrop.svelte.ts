// Files dropped onto a pane.
//
// Tauri takes files dropped on the window before the webview sees them and
// reports them as its own drag-drop events, with the file paths and the
// pointer position in physical pixels; HTML drop events never fire. This
// module follows those events, highlights the pane under the pointer while
// files are dragged (PaneView reads `fileDrop.sessionId`) and, on drop,
// types the files' paths into that pane:
//
// 1. A session on this computer gets the dropped paths themselves.
// 2. A remote session gets the paths of copies: each file is first copied
//    to the server (`upload_drops`, app/src-tauri/src/drops.rs), because a
//    path on this computer means nothing there.
//
// The paths are typed with the terminal's paste, so a program that turned
// on bracketed paste (Claude Code does) receives them as one paste.
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { errorText } from "../errors";
import { isMac } from "../platform";
import { focusPane, machineOfSession } from "../state/app.svelte";
import { showNotice } from "../state/notices.svelte";
import { terminalControls } from "../terminal/registry";
import { LOCAL_MACHINE } from "../wire/bridge";
import { dropText } from "./dropPaths";

/** The pane the dragged files are over, or null when they are over no pane. */
export const fileDrop = $state({ sessionId: null as string | null });

/** How long the "Copying" notice may stay while a remote upload runs. */
const UPLOAD_NOTICE_MS = 120_000;

/** The session whose pane (`data-drop-session`) is at a position given in physical pixels. */
function paneAt(position: { x: number; y: number }): string | null {
  const scale = window.devicePixelRatio || 1;
  const element = document.elementFromPoint(position.x / scale, position.y / scale);
  return element?.closest<HTMLElement>("[data-drop-session]")?.dataset.dropSession ?? null;
}

/** Types `paths` into the session's pane, copying them to its server first when it is remote. */
export async function dropFiles(sessionId: string, paths: string[]): Promise<void> {
  if (paths.length === 0) return;
  focusPane(sessionId);
  const machineId = machineOfSession(sessionId);
  let typed = paths;
  let style: "windows" | "posix" = isMac ? "posix" : "windows";
  if (machineId !== LOCAL_MACHINE) {
    const what = paths.length === 1 ? "1 file" : `${paths.length} files`;
    showNotice(sessionId, `Copying ${what} to the server…`, "info", UPLOAD_NOTICE_MS);
    try {
      typed = await invoke<string[]>("upload_drops", { id: machineId, paths });
    } catch (err) {
      showNotice(sessionId, errorText(err), "error");
      return;
    }
    showNotice(sessionId, `Copied ${what} to ~/.jojo/drops`);
    style = "posix";
  }
  terminalControls(sessionId)?.paste(dropText(typed, style));
}

/** Starts following the window's drag-drop events; returns the function that stops it. */
export function startFileDrop(): () => void {
  let stopped = false;
  let unlisten: (() => void) | null = null;
  void getCurrentWebview()
    .onDragDropEvent((event) => {
      const payload = event.payload;
      if (payload.type === "enter" || payload.type === "over") {
        fileDrop.sessionId = paneAt(payload.position);
      } else if (payload.type === "drop") {
        const sessionId = paneAt(payload.position);
        fileDrop.sessionId = null;
        if (sessionId) void dropFiles(sessionId, payload.paths);
      } else {
        fileDrop.sessionId = null;
      }
    })
    .then((stop) => {
      if (stopped) stop();
      else unlisten = stop;
    })
    .catch(() => {
      // Outside Tauri (unit tests, a browser preview) there are no drop events.
    });
  return () => {
    stopped = true;
    unlisten?.();
  };
}
