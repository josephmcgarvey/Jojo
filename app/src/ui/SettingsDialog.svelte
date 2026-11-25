<script lang="ts">
  // Spec 9.10: the settings screen. It edits a copy of the settings and
  // saves them with Save; terminal font changes apply to open panes at once,
  // scrollback to panes opened afterwards. It also offers Restart daemon
  // (This PC) and Debug ▸ Rehydrate pane. The theme, base color and interface
  // font preview live while the screen is open; Cancel restores the saved ones.
  import { onMount } from "svelte";
  import type { Settings } from "../lib/backend/Settings";
  import { errorText } from "../lib/errors";
  import { TERMINAL_FONTS, UI_FONTS, fontLabel, terminalFontStack } from "../lib/fonts";
  import { LOCAL_LABEL, OS_NAME, isMac } from "../lib/platform";
  import { BASE_COLOR_PRESETS, effectiveAccent, parseHex } from "../lib/palette";
  import { app, confirmRestartLocalDaemon, focusedSessionId, rehydrateFocusedPane, runningSessions, saveSettings } from "../lib/state/app.svelte";
  import { shellChoices } from "../lib/state/shells";
  import { chooseConfigImport, exportConfig } from "../lib/state/configTransfer";
  import { openDialog } from "../lib/state/ui.svelte";
  import { THEMES, themeById } from "../lib/theme";
  import { LOCAL_MACHINE } from "../lib/wire/bridge";

  let { onclose }: { onclose: () => void } = $props();

  const DEFAULTS: Settings = {
    theme: "neon-glass",
    terminalFontFamily: "Geist Mono",
    terminalFontSize: 13,
    terminalLineHeight: 1.25,
    scrollbackLines: 10_000,
    copyOnSelect: true,
    toasts: true,
    defaultShell: isMac ? "/bin/zsh" : "pwsh",
    promptLabels: false,
    sound: true,
    restoreSessions: true,
    uiFontFamily: "",
    accentColor: "",
    sidebarWidth: 268,
    sidebarHidden: false,
  };
  /** The terminal font select's value for a family typed into the text field. */
  const OTHER_FONT = "__other__";

  let draft = $state<Settings>({ ...DEFAULTS, ...(app.settings ?? {}) });
  $effect(() => {
    const match = app.shells.find((s) => s.program !== draft.defaultShell && sameProgram(s.program, draft.defaultShell));
    if (match) draft.defaultShell = match.program;
  });
  let otherFont = $state(!TERMINAL_FONTS.some((f) => f.family === draft.terminalFontFamily));
  const fontChoice = $derived(otherFont ? OTHER_FONT : draft.terminalFontFamily);
  function chooseFont(value: string): void {
    otherFont = value === OTHER_FONT;
    if (!otherFont) draft.terminalFontFamily = value;
  }
  const theme = $derived(themeById(draft.theme));
  /** The theme's own interface font, named in the Theme default option: the first family of its font-ui token. */
  const themeFont = $derived(/^\s*"?([^",]+)/.exec(theme.tokens["font-ui"] ?? "")?.[1] ?? "Geist Sans");
  /** The base color in use: the chosen one clamped to the theme's lightness range, or the theme's own. */
  const accent = $derived(effectiveAccent(theme, draft.accentColor));
  // A preset equal to the theme's own accent (Violet on Neon Glass and Prism)
  // is left out: the default swatch already draws that color.
  const swatches = $derived([
    { name: `${theme.name} default`, value: "", shown: theme.accent },
    ...BASE_COLOR_PRESETS.filter((p) => p.color.toLowerCase() !== theme.accent.toLowerCase()).map((p) => ({
      name: p.name,
      value: p.color,
      shown: effectiveAccent(theme, p.color),
    })),
  ]);
  /**
   * The swatch whose stored value is the chosen base color: the default when
   * none is set (or it is not a #rgb/#rrggbb color), a preset when it is that
   * preset's color, none for any other color. Compared by stored value, not by
   * the color drawn, because two presets can be adjusted to the same color on
   * a light theme and a preset can equal the theme's accent.
   */
  const chosenSwatch = $derived.by(() => {
    const value = draft.accentColor.trim().toLowerCase();
    if (!parseHex(value)) return "";
    return swatches.find((s) => s.value !== "" && s.value.toLowerCase() === value)?.value ?? (value === theme.accent.toLowerCase() ? "" : null);
  });
  // Live preview of the appearance choices; cleared when the screen closes.
  $effect(() => {
    app.themePreview = { theme: draft.theme, accentColor: draft.accentColor, uiFontFamily: draft.uiFontFamily };
  });
  onMount(() => () => {
    app.themePreview = null;
  });
  let error = $state<string | null>(null);
  let saving = $state(false);
  // Ordered by the saved default, so the list does not reorder while choosing.
  const shells = $derived(shellChoices(app.shells, app.settings?.defaultShell));
  /** Names the same program as a detected shell (`pwsh.exe` and `pwsh`, any case). */
  const sameProgram = (a: string, b: string) => a.trim().toLowerCase().replace(/\.exe$/, "") === b.trim().toLowerCase().replace(/\.exe$/, "");
  let dialogElement: HTMLDivElement;
  onMount(() => dialogElement.focus());
  const localSessions = $derived(runningSessions(LOCAL_MACHINE).length);
  const canRehydrate = $derived(focusedSessionId() !== null);

  /** Keeps numbers inside the ranges xterm.js and the daemon accept. */
  function clamped(value: number, min: number, max: number, fallback: number): number {
    return Number.isFinite(value) ? Math.min(max, Math.max(min, value)) : fallback;
  }

  async function save(): Promise<void> {
    saving = true;
    error = null;
    try {
      await saveSettings({
        ...draft,
        terminalFontFamily: draft.terminalFontFamily.trim() || DEFAULTS.terminalFontFamily,
        uiFontFamily: draft.uiFontFamily.trim() || DEFAULTS.uiFontFamily,
        accentColor: parseHex(draft.accentColor) ? draft.accentColor.toLowerCase() : "",
        terminalFontSize: Math.round(clamped(draft.terminalFontSize, 8, 32, DEFAULTS.terminalFontSize)),
        terminalLineHeight: clamped(draft.terminalLineHeight, 1, 2, DEFAULTS.terminalLineHeight),
        scrollbackLines: Math.round(clamped(draft.scrollbackLines, 10_000, 1_000_000, DEFAULTS.scrollbackLines)),
      });
      onclose();
    } catch (err) {
      error = errorText(err);
    } finally {
      saving = false;
    }
  }

  /** The result of the last Export configuration…, shown under the buttons. */
  let transfer = $state<{ text: string; tone: "info" | "error" } | null>(null);

  async function exportFile(): Promise<void> {
    transfer = null;
    try {
      const path = await exportConfig();
      if (path) transfer = { text: `Saved ${path}.`, tone: "info" };
    } catch (err) {
      transfer = { text: errorText(err), tone: "error" };
    }
  }

  /** Reads the chosen file and replaces this screen with the import preview; unsaved changes here are discarded. */
  async function importFile(): Promise<void> {
    transfer = null;
    try {
      const chosen = await chooseConfigImport();
      if (chosen) openDialog({ kind: "import-config", ...chosen });
    } catch (err) {
      transfer = { text: errorText(err), tone: "error" };
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") onclose();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="scrim jojo-scrim" onkeydown={onKeydown}>
  <div class="dialog jojo-modal" role="dialog" aria-modal="true" aria-labelledby="settings-title" tabindex="-1" bind:this={dialogElement}>
    <h2 id="settings-title">Settings</h2>

    <section>
      <h3>Terminal</h3>
      <label>
        <span>Font</span>
        <select value={fontChoice} onchange={(e) => chooseFont(e.currentTarget.value)}>
          {#each TERMINAL_FONTS as font (font.family)}
            <option value={font.family}>{fontLabel(font)}</option>
          {/each}
          <option value={OTHER_FONT}>Other installed font…</option>
        </select>
      </label>
      {#if otherFont}
        <label><span>Font name</span><input bind:value={draft.terminalFontFamily} spellcheck="false" placeholder="Font family name, for example Hack" /></label>
      {/if}
      <p class="sample" style:font-family={terminalFontStack(draft.terminalFontFamily)}>claude --resume  0O 1lI {"{}"} [] =&gt; ~/.jojo</p>
      <label><span>Font size</span><input type="number" min="8" max="32" step="1" bind:value={draft.terminalFontSize} /></label>
      <label><span>Line height</span><input type="number" min="1" max="2" step="0.05" bind:value={draft.terminalLineHeight} /></label>
      <label><span>Scrollback lines</span><input type="number" min="10000" max="1000000" step="1000" bind:value={draft.scrollbackLines} /></label>
      <label class="check"><input type="checkbox" bind:checked={draft.copyOnSelect} /><span>Copy on select (Ctrl+C always reaches the terminal)</span></label>
      <label>
        <span>Default shell</span>
        <select bind:value={draft.defaultShell}>
          {#each shells as shell (shell.program)}
            <option value={shell.program}>{shell.label.replace(" (default)", "")}</option>
          {/each}
          {#if app.shells.length > 0 && !shells.some((s) => sameProgram(s.program, draft.defaultShell))}
            <option value={draft.defaultShell}>{draft.defaultShell} (not found)</option>
          {/if}
        </select>
      </label>
    </section>

    <section>
      <h3>Appearance and notifications</h3>
      <label>
        <span>Theme</span>
        <select bind:value={draft.theme}>
          {#each Object.entries(THEMES) as [id, t] (id)}
            <option value={id}>{t.name} ({t.mode})</option>
          {/each}
        </select>
      </label>
      <div class="field">
        <span>Base color</span>
        <div class="swatches">
          {#each swatches as swatch (swatch.name)}
            <button
              class="swatch"
              class:chosen={swatch.value === chosenSwatch}
              title={swatch.name}
              aria-label={`Base color ${swatch.name}`}
              aria-pressed={swatch.value === chosenSwatch}
              style:background={swatch.shown}
              onclick={() => (draft.accentColor = swatch.value)}
            ></button>
          {/each}
          <input
            type="color"
            title="Any color"
            aria-label="Any base color"
            value={accent}
            oninput={(e) => (draft.accentColor = e.currentTarget.value)}
          />
        </div>
      </div>
      <p class="note indent">
        Panels, lines, text tints and the terminal background follow the base color. The state colors (working, needs you, done, error) do not change.
        {#if parseHex(draft.accentColor) && accent !== draft.accentColor.toLowerCase()}Lightness is adjusted to {accent} to stay readable on {theme.name}.{/if}
      </p>
      <label>
        <span>Interface font</span>
        <select bind:value={draft.uiFontFamily}>
          <option value="">Theme default ({themeFont})</option>
          {#each UI_FONTS as font (font.family)}
            <option value={font.family}>{fontLabel(font)}</option>
          {/each}
          {#if draft.uiFontFamily && !UI_FONTS.some((f) => f.family === draft.uiFontFamily)}
            <option value={draft.uiFontFamily}>{draft.uiFontFamily}</option>
          {/if}
        </select>
      </label>
      <label class="check"><input type="checkbox" bind:checked={draft.toasts} /><span>{OS_NAME} notifications for sessions that need you, finish or fail</span></label>
      <label class="check"><input type="checkbox" bind:checked={draft.sound} /><span>Play a chime when a session finishes or needs a response</span></label>
      <label class="check"><input type="checkbox" bind:checked={draft.restoreSessions} /><span>Restore sessions after {OS_NAME} or jojod restarts (Claude conversations resume)</span></label>
      <label class="check"><input type="checkbox" bind:checked={draft.promptLabels} /><span>Name sessions from Claude's first prompt (until you rename them)</span></label>
    </section>

    <section>
      <h3>Configuration</h3>
      <div class="row">
        <button onclick={() => void exportFile()}>Export configuration…</button>
        <span class="note">Saves settings, remote locations, snippets and remote tab layouts to a jojo-config.json file. Passwords and SSH keys are not included.</span>
      </div>
      <div class="row">
        <button onclick={() => void importFile()}>Import configuration…</button>
        <span class="note">Shows what a jojo-config.json file would change before anything is saved. Unsaved changes on this screen are discarded.</span>
      </div>
      {#if transfer}<p class="transfer" class:error={transfer.tone === "error"}>{transfer.text}</p>{/if}
    </section>

    <section>
      <h3>Maintenance</h3>
      <div class="row">
        <button onclick={() => void confirmRestartLocalDaemon()}>Restart daemon ({LOCAL_LABEL})</button>
        <span class="note">
          Ends {localSessions} running session{localSessions === 1 ? "" : "s"}. Sessions started afterwards see PATH changes made since jojod started.
        </span>
      </div>
      <div class="row">
        <button onclick={rehydrateFocusedPane} disabled={!canRehydrate}>Debug ▸ Rehydrate pane</button>
        <span class="note">Repaints the focused pane from a new snapshot, if its screen looks wrong.</span>
      </div>
    </section>

    {#if error}<p class="error">{error}</p>{/if}
    <div class="actions">
      <!-- The app's version and This PC's daemon version, when connected. -->
      <span class="version">
        {#if app.version}Jojo {app.version}{/if}{#if app.status.state === "connected"} · jojod {app.status.jojod_version}{/if}
      </span>
      <button onclick={onclose}>Cancel</button>
      <button class="primary" onclick={save} disabled={saving}>Save</button>
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: grid;
    place-items: center;
  }
  .dialog {
    outline: none;
    width: min(560px, calc(100% - 32px));
    max-height: calc(100% - 32px);
    overflow-y: auto;
    padding: 18px 20px 16px;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 14px;
    font-weight: 600;
    color: var(--jojo-fg);
  }
  h3 {
    margin: 14px 0 8px;
    font-size: 10.5px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--jojo-muted);
  }
  label {
    display: grid;
    grid-template-columns: 130px 1fr;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
    color: var(--jojo-fg2);
  }
  label.check {
    grid-template-columns: 18px 1fr;
  }
  input:not([type="checkbox"]),
  select {
    padding: 5px 8px;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    background: var(--jojo-pane);
    color: var(--jojo-fg);
    font: inherit;
  }
  .row {
    display: grid;
    grid-template-columns: 190px 1fr;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }
  .row button,
  .actions button {
    padding: 5px 12px;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    color: var(--jojo-fg2);
  }
  .row button:disabled {
    opacity: 0.5;
  }
  .note {
    color: var(--jojo-muted);
    font-size: 12px;
  }
  .note.indent,
  .sample {
    margin: -2px 0 10px 138px;
  }
  .sample {
    padding: 6px 8px;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    background: var(--jojo-pane);
    color: var(--jojo-fg);
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
  }
  .field {
    display: grid;
    grid-template-columns: 130px 1fr;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
    color: var(--jojo-fg2);
  }
  .swatches {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }
  .swatch {
    width: 22px;
    height: 22px;
    border-radius: 50%;
    border: 2px solid transparent;
    box-shadow: inset 0 0 0 1px var(--jojo-line);
  }
  .swatch.chosen {
    border-color: var(--jojo-fg);
  }
  input[type="color"] {
    width: 30px;
    height: 24px;
    padding: 0;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    background: none;
  }
  .transfer {
    margin: 0 0 8px 200px;
    color: var(--jojo-muted);
    font-size: 12px;
    overflow-wrap: anywhere;
  }
  .error {
    color: var(--jojo-error);
    font-size: 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    align-items: center;
    gap: 8px;
    margin-top: 14px;
  }
  .version {
    margin-right: auto;
    color: var(--jojo-muted);
    font-size: 11.5px;
    font-variant-numeric: tabular-nums;
    user-select: text;
  }
  .actions .primary {
    border-color: var(--jojo-accent);
    background: var(--jojo-accent);
    color: var(--jojo-bg);
  }
</style>
