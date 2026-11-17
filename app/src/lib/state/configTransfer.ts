// Settings ▸ Export configuration… and Import configuration…: the file
// dialogs, and applying an import's merged settings, snippets and layouts
// to the window's state (see app/src-tauri/src/config.rs for what the file
// holds and the merge rules).
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { ImportPreview } from "../backend/ImportPreview";
import type { ImportResult } from "../backend/ImportResult";
import { machineAdded, mergeImportedLayouts, saveSettings } from "./app.svelte";
import { replaceSnippets } from "./snippets.svelte";

const FILTERS = [{ name: "Jojo configuration", extensions: ["json"] }];

/** Asks where to save the file and writes it; the path, or null when the user cancelled. */
export async function exportConfig(): Promise<string | null> {
  const path = await save({ defaultPath: "jojo-config.json", filters: FILTERS });
  if (!path) return null;
  await invoke("export_config", { path });
  return path;
}

/** Asks for a configuration file and reads what importing it would change; null when the user cancelled. */
export async function chooseConfigImport(): Promise<{ path: string; preview: ImportPreview } | null> {
  const path = await open({ multiple: false, directory: false, filters: FILTERS });
  if (typeof path !== "string") return null;
  return { path, preview: await invoke<ImportPreview>("preview_config_import", { path }) };
}

/**
 * Imports the file. The backend adds the new locations and saves their
 * passwords (keyed by the location ids in the file); the window then adds
 * the layouts, replaces the snippets and saves the settings, so its own
 * state does not overwrite the import on its next save.
 */
export async function importConfig(path: string, passwords: Record<string, string>): Promise<ImportResult> {
  const result = await invoke<ImportResult>("import_config", { path, passwords });
  mergeImportedLayouts(result.layouts, result.project_order);
  for (const machine of result.machines) machineAdded(machine);
  replaceSnippets(result.snippets);
  await saveSettings(result.settings);
  return result;
}
