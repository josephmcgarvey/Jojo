// Copy and paste for one pane, through the WebView2 clipboard API (main.rs
// enables clipboard access on the webview so readText is allowed).
import { errorText } from "../errors";
import { terminalControls } from "../terminal/registry";
import { showNotice } from "./notices.svelte";

/**
 * Copies the pane's selection. xterm.js getSelection() joins wrapped lines
 * and trims trailing whitespace (spec 9.5 rule 1). Does nothing when
 * nothing is selected.
 */
export async function copyFromPane(sessionId: string): Promise<void> {
  const text = terminalControls(sessionId)?.getSelection() ?? "";
  if (text === "") return;
  try {
    await navigator.clipboard.writeText(text);
    showNotice(sessionId, "Copied");
  } catch (err) {
    showNotice(sessionId, `Copy failed: ${errorText(err)}`, "error");
  }
}

/** Pastes the clipboard text into the pane; xterm.js applies bracketed paste when the program enabled it. */
export async function pasteIntoPane(sessionId: string): Promise<void> {
  const controls = terminalControls(sessionId);
  if (!controls) return;
  try {
    const text = await navigator.clipboard.readText();
    if (text !== "") controls.paste(text);
  } catch (err) {
    showNotice(sessionId, `Paste failed: ${errorText(err)}`, "error");
  }
}
