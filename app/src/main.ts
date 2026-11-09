import "@fontsource/geist-sans/400.css";
import "@fontsource/geist-sans/500.css";
import "@fontsource/geist-sans/600.css";
import "@fontsource/geist-mono/400.css";
import "@fontsource/geist-mono/700.css";
import "@fontsource/jetbrains-mono/400.css";
import "@fontsource/jetbrains-mono/700.css";
import "@fontsource/fira-code/400.css";
import "@fontsource/fira-code/700.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/700.css";
import "@fontsource/inter/400.css";
import "@fontsource/inter/500.css";
import "@fontsource/inter/600.css";
import "@xterm/xterm/css/xterm.css";
import "./app.css";
import { mount } from "svelte";
import App from "./App.svelte";
import { TERMINAL_FONTS, loadTerminalFont } from "./lib/fonts";
import { isMac } from "./lib/platform";

/**
 * xterm.js measures its character cell once, when a terminal opens, and the
 * WebGL renderer caches the glyphs it drew. The @fontsource faces use
 * `font-display: swap` and load only when first used, so a pane opened before
 * Geist Mono has loaded is measured with a fallback font and then draws Geist
 * Mono glyphs inside the fallback's wider cells. Loading both terminal weights
 * (400 regular, 700 bold) of every bundled terminal font before mounting the
 * app prevents that; the settings, which name the chosen font, are not read
 * yet. A failed load still mounts the app, with the fallback font.
 */
async function loadTerminalFonts(): Promise<void> {
  await Promise.all(TERMINAL_FONTS.filter((f) => f.source === "bundled").map((f) => loadTerminalFont(f.family)));
}

// macOS draws its traffic-light buttons over the sidebar's top row
// (titleBarStyle Overlay in tauri.macos.conf.json); CSS keyed on this
// attribute makes room for them.
if (isMac) document.documentElement.dataset.platform = "mac";

loadTerminalFonts().then(() => {
  mount(App, { target: document.getElementById("app")! });
});
