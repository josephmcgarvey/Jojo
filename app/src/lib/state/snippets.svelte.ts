// The snippets library in the window: loaded from snippets.json at start,
// saved after every change, and inserted into or run in the focused
// terminal (see lib/snippets/library.ts for the rules).
import { invoke } from "@tauri-apps/api/core";
import type { Snippet } from "../backend/Snippet";
import type { SnippetScope } from "../backend/SnippetScope";
import { errorText } from "../errors";
import { fillPlaceholders, insertionText, placeholders, type SnippetTarget } from "../snippets/library";
import { terminalControls } from "../terminal/registry";
import { sendInput } from "../wire/bridge";
import { newUlid } from "../wire/ulid";
import { app, displayLabel, focusedSessionId, machineOfSession } from "./app.svelte";
import { askConfirm } from "./dialog.svelte";
import { showNotice } from "./notices.svelte";
import { openDialog } from "./ui.svelte";

export const snippets = $state({
  list: [] as Snippet[],
  /** True once snippets.json has been read; nothing is written before that, so a failed read never empties the file. */
  loaded: false,
});

let saveChain: Promise<unknown> = Promise.resolve();

export function loadSnippets(): void {
  void invoke<Snippet[]>("get_snippets")
    .then((list) => {
      snippets.list = list;
      snippets.loaded = true;
    })
    .catch(() => {
      // The library stays empty and unsaved for this run.
    });
}

/** Saves the library; calls are chained so two writes never overlap. */
function save(): void {
  if (!snippets.loaded) return;
  const list = $state.snapshot(snippets.list);
  saveChain = saveChain.then(() => invoke("save_snippets", { snippets: list })).catch(() => {});
}

export interface SnippetFields {
  name: string;
  command: string;
  folder: string;
  scope: SnippetScope;
}

export function addSnippet(fields: SnippetFields): void {
  snippets.list = [...snippets.list, { id: newUlid(), last_values: {}, ...fields }];
  save();
}

export function updateSnippet(id: string, fields: SnippetFields): void {
  snippets.list = snippets.list.map((s) => (s.id === id ? { ...s, ...fields } : s));
  save();
}

export async function deleteSnippet(id: string): Promise<void> {
  const snippet = snippets.list.find((s) => s.id === id);
  if (!snippet) return;
  const confirmed = await askConfirm({ title: "Delete snippet?", message: `“${snippet.name}” is removed from the library.`, details: [], confirmLabel: "Delete" });
  if (!confirmed) return;
  snippets.list = snippets.list.filter((s) => s.id !== id);
  save();
}

/** The focused terminal's location and project, or null when no terminal is focused. */
export function snippetTarget(): SnippetTarget | null {
  const sessionId = focusedSessionId();
  const session = sessionId ? app.model.sessions[sessionId] : undefined;
  return session ? { machineId: machineOfSession(session.id), projectId: session.project_id } : null;
}

/** The focused terminal's name, for "Inserts into …"; null when no terminal is focused. */
export function snippetTargetLabel(): string | null {
  const sessionId = focusedSessionId();
  const session = sessionId ? app.model.sessions[sessionId] : undefined;
  return session ? displayLabel(session) : null;
}

/** Delay between the snippet's text and Enter for Run (as Compact conversation). */
const RUN_ENTER_DELAY_MS = 80;

/**
 * Types `command` into the focused terminal and focuses it. The text goes
 * through xterm.js's paste, so a shell that enabled bracketed paste
 * receives a multi-line command as one block instead of running each line
 * as it arrives. With `run`, Enter follows in a separate write after
 * RUN_ENTER_DELAY_MS; Enter inside the paste would be read as part of the
 * pasted text.
 */
function typeIntoFocused(command: string, run: boolean): void {
  const sessionId = focusedSessionId();
  const controls = terminalControls(sessionId);
  if (!sessionId || !controls) return;
  controls.paste(insertionText(command));
  controls.focus();
  if (!run) return;
  const machine = machineOfSession(sessionId);
  setTimeout(() => void sendInput(sessionId, new TextEncoder().encode("\r"), machine).catch(() => {}), RUN_ENTER_DELAY_MS);
}

/**
 * Inserts (or with `run`, runs) a snippet in the focused terminal. A
 * snippet whose command has `{{name}}` placeholders first opens the form
 * that asks for their values (SnippetFillDialog.svelte), which then calls
 * fillAndInsertSnippet.
 */
export function insertSnippet(id: string, run: boolean): void {
  const snippet = snippets.list.find((s) => s.id === id);
  if (!snippet || !terminalControls(focusedSessionId())) return;
  if (placeholders(snippet.command).length > 0) openDialog({ kind: "snippet-fill", snippetId: id, run });
  else typeIntoFocused(snippet.command, run);
}

/**
 * Fills a snippet's placeholders with `values`, remembers the values as the
 * snippet's last_values (saved in snippets.json) and types the result.
 */
export function fillAndInsertSnippet(id: string, values: Record<string, string>, run: boolean): void {
  const snippet = snippets.list.find((s) => s.id === id);
  if (!snippet) return;
  snippets.list = snippets.list.map((s) => (s.id === id ? { ...s, last_values: { ...s.last_values, ...values } } : s));
  save();
  typeIntoFocused(fillPlaceholders(snippet.command, values), run);
}

/** Copies the snippet's command to the clipboard, with a notice on the focused pane when there is one. */
export async function copySnippet(id: string): Promise<void> {
  const snippet = snippets.list.find((s) => s.id === id);
  if (!snippet) return;
  const sessionId = focusedSessionId();
  try {
    await navigator.clipboard.writeText(snippet.command);
    if (sessionId) showNotice(sessionId, `Copied “${snippet.name}”`);
  } catch (err) {
    if (sessionId) showNotice(sessionId, `Copy failed: ${errorText(err)}`, "error");
  }
}
