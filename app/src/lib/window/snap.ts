// The maximize button's rectangle in physical pixels, for the snap-layout
// input sink (app/src-tauri/src/snap.rs). Each edge is scaled and rounded
// separately, so the sink covers the same pixels the button is drawn on.

export interface PhysicalRect {
  x: number;
  y: number;
  width: number;
  height: number;
}

export function toPhysicalRect(rect: { left: number; top: number; width: number; height: number }, scale: number): PhysicalRect {
  const x = Math.round(rect.left * scale);
  const y = Math.round(rect.top * scale);
  return {
    x,
    y,
    width: Math.round((rect.left + rect.width) * scale) - x,
    height: Math.round((rect.top + rect.height) * scale) - y,
  };
}
