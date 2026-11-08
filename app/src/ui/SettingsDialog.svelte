<script lang="ts">
  // Spec 9.10: the settings screen. It edits a copy of the settings and
  // saves them with Save; terminal font changes apply to open panes at once,
  // scrollback to panes opened afterwards. It also offers Restart daemon
  // (This PC) and Debug ▸ Rehydrate pane.
  import { onMount } from "svelte";
  import type { Settings } from "../lib/backend/Settings";
  import { errorText } from "../lib/errors";
  import { app, confirmRestartLocalDaemon, focusedSessionId, rehydrateFocusedPane, runningSessions, saveSettings } from "../lib/state/app.svelte";
  import { shellChoices } from "../lib/state/shells";
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
    defaultShell: "pwsh",
  };

  let draft = $state<Settings>({ ...DEFAULTS, ...(app.settings ?? {}) });
  $effect(() => {
    const match = app.shells.find((s) => s.program !== draft.defaultShell && sameProgram(s.program, draft.defaultShell));
    if (match) draft.defaultShell = match.program;
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

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") onclose();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="scrim" onkeydown={onKeydown}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="settings-title" tabindex="-1" bind:this={dialogElement}>
    <h2 id="settings-title">Settings</h2>

    <section>
      <h3>Terminal</h3>
      <label><span>Font family</span><input bind:value={draft.terminalFontFamily} spellcheck="false" /></label>
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
        <select bind:value={draft.theme}><option value="neon-glass">Neon Glass</option></select>
      </label>
      <label class="check"><input type="checkbox" bind:checked={draft.toasts} /><span>Windows notifications for sessions that need you, finish or fail</span></label>
    </section>

    <section>
      <h3>Maintenance</h3>
      <div class="row">
        <button onclick={() => void confirmRestartLocalDaemon()}>Restart daemon (This PC)</button>
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
    background: var(--jojo-scrim);
  }
  .dialog {
    outline: none;
    width: min(560px, calc(100% - 32px));
    max-height: calc(100% - 32px);
    overflow-y: auto;
    padding: 18px 20px 16px;
    background: var(--jojo-menu);
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius);
    box-shadow: var(--jojo-shadow);
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
    border: 1px solid var(--jojo-line);
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
    border: 1px solid var(--jojo-line);
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
  .error {
    color: var(--jojo-error);
    font-size: 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
  }
  .actions .primary {
    border-color: var(--jojo-accent);
    background: var(--jojo-accent);
    color: var(--jojo-bg);
  }
</style>
