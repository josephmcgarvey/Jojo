// jojod rejects sizes outside 2-1000 columns and 1-1000 rows; a hidden pane
// reports no size at all.
export function clampSize(cols: number, rows: number): { cols: number; rows: number } {
  const clamp = (v: number, min: number) => (Number.isFinite(v) ? Math.min(1000, Math.max(min, Math.floor(v))) : min);
  return { cols: clamp(cols, 2), rows: clamp(rows, 1) };
}
