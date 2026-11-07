/**
 * Text for the reason a promise rejected. Tauri commands reject with the
 * command's error string; DOM APIs (clipboard) reject with Error objects.
 */
export function errorText(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}
