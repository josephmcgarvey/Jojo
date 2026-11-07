import "@fontsource/geist-sans/400.css";
import "@fontsource/geist-sans/500.css";
import "@fontsource/geist-sans/600.css";
import "@fontsource/geist-mono/400.css";
import "@fontsource/geist-mono/700.css";
import "@xterm/xterm/css/xterm.css";
import "./app.css";
import { mount } from "svelte";
import App from "./App.svelte";

/**
 * xterm.js measures its character cell once, when a terminal opens, and the
 * WebGL renderer caches the glyphs it drew. The @fontsource faces use
 * `font-display: swap` and load only when first used, so a pane opened before
 * Geist Mono has loaded is measured with a fallback font and then draws Geist
 * Mono glyphs inside the fallback's wider cells. Loading both terminal weights
 * (400 regular, 700 bold) before mounting the app prevents that. A failed load
 * still mounts the app, with the fallback font.
 */
async function loadTerminalFonts(): Promise<void> {
  try {
    await Promise.all([
      document.fonts.load('400 13px "Geist Mono"'),
      document.fonts.load('700 13px "Geist Mono"'),
    ]);
  } catch {
    // The terminal falls back to the next available monospace font.
  }
}

loadTerminalFonts().then(() => {
  mount(App, { target: document.getElementById("app")! });
});
