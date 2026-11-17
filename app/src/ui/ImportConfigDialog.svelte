<script lang="ts">
  // Settings ▸ Import configuration…: shows what importing a jojo-config.json
  // file would change, asks for the password of each new location that
  // signs in with one, and imports it. Nothing is written before Import.
  import type { ImportPreview } from "../lib/backend/ImportPreview";
  import type { LocationPlan } from "../lib/backend/LocationPlan";
  import { errorText } from "../lib/errors";
  import { isMac } from "../lib/platform";
  import { importConfig } from "../lib/state/configTransfer";

  let { path, preview, onclose }: { path: string; preview: ImportPreview; onclose: () => void } = $props();

  /** Settings keys as the Settings screen names them. */
  const SETTING_NAMES: Record<string, string> = {
    theme: "Theme",
    terminalFontFamily: "Terminal font",
    terminalFontSize: "Font size",
    terminalLineHeight: "Line height",
    scrollbackLines: "Scrollback lines",
    copyOnSelect: "Copy on select",
    toasts: "Notifications",
    promptLabels: "Name sessions from Claude's first prompt",
    sound: "Chime",
    restoreSessions: "Restore sessions",
    uiFontFamily: "Interface font",
    accentColor: "Base color",
    sidebarWidth: "Sidebar width",
  };
  const CREDENTIAL_STORE = isMac ? "the Keychain" : "Windows Credential Manager";

  const fileName = $derived(path.split(/[\\/]/).pop() ?? path);
  const exported = $derived(new Date(preview.exported_at).toLocaleString());
  const adding = $derived(preview.locations.filter((l) => l.action === "add"));
  const listed = $derived(preview.locations.filter((l) => l.action === "exists"));
  const nothing = $derived(
    adding.length === 0 && preview.snippets_added + preview.snippets_replaced === 0 && preview.settings_changed.length === 0 && preview.layouts_added === 0,
  );

  // svelte-ignore state_referenced_locally
  let passwords = $state<Record<string, string>>(Object.fromEntries(preview.locations.filter((l) => l.needs_password).map((l) => [l.id, ""])));
  let running = $state(false);
  let done = $state(false);
  let error = $state<string | null>(null);
  let notes = $state<string[]>([]);
  let added = $state(0);

  function address(l: LocationPlan): string {
    return l.port ? `${l.ssh_host} port ${l.port}` : l.ssh_host;
  }

  async function run(): Promise<void> {
    running = true;
    error = null;
    try {
      const result = await importConfig(path, $state.snapshot(passwords));
      passwords = {};
      notes = result.notes;
      added = result.machines.length;
      done = true;
    } catch (err) {
      error = errorText(err);
    } finally {
      running = false;
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape" && !running) onclose();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="scrim jojo-scrim" onkeydown={onKeydown}>
  <div class="dialog jojo-modal" role="dialog" aria-modal="true" aria-labelledby="import-config-title">
    <h2 id="import-config-title">Import configuration</h2>
    <p class="hint"><code>{fileName}</code>, exported from Jojo {preview.app_version} on {exported}.</p>

    {#if done}
      <p>
        The configuration was imported.
        {#if added > 0}{added} remote location{added === 1 ? " was" : "s were"} added and {added === 1 ? "is" : "are"} connecting.{/if}
      </p>
      {#if notes.length > 0}
        <ul class="notes">
          {#each notes as note, i (i)}<li>{note}</li>{/each}
        </ul>
      {/if}
    {:else}
      {#if preview.locations.length > 0}
        <h3>Remote locations</h3>
        <ul class="locations">
          {#each adding as l (l.id)}
            <li>
              <div class="line"><span class="label">{l.label}</span><span class="address">{address(l)}</span><span class="tag">Added</span></div>
              {#if l.needs_password}
                <label class="password">
                  <span>Password</span>
                  <input type="password" bind:value={passwords[l.id]} autocomplete="off" disabled={running} aria-label={`Password for ${l.ssh_host}`} />
                </label>
                <p class="note">Saved in {CREDENTIAL_STORE}. Leave it empty to skip this location.</p>
              {/if}
              {#if l.missing_alias}
                <p class="warn">This computer's <code>~/.ssh/config</code> has no Host entry for <code>{l.ssh_host}</code>. Copy the entry and its key from the other computer, or the location cannot connect.</p>
              {/if}
              {#if l.missing_key}
                <p class="warn">This location signs in with <code>~/.ssh/jojo_ed25519</code>, which this computer does not have. Copy <code>jojo_ed25519</code> and <code>jojo_ed25519.pub</code> from the other computer's <code>~/.ssh</code> folder, or remove the location after importing and add it again with its username and password.</p>
              {/if}
            </li>
          {/each}
          {#each listed as l (l.id)}
            <li>
              <div class="line"><span class="label">{l.label}</span><span class="address">{address(l)}</span><span class="tag muted">Already listed</span></div>
            </li>
          {/each}
        </ul>
      {/if}

      <h3>Other changes</h3>
      <ul class="summary">
        <li>
          Snippets: {preview.snippets_added} new{#if preview.snippets_replaced > 0}, {preview.snippets_replaced} replacing older copies with the same id{/if}.
        </li>
        <li>
          Settings:
          {#if preview.settings_changed.length > 0}{preview.settings_changed.map((key) => SETTING_NAMES[key] ?? key).join(", ")}.{:else}no changes.{/if}
          The default shell is not imported.
        </li>
        <li>Tabs for {preview.layouts_added} remote project{preview.layouts_added === 1 ? "" : "s"} that {preview.layouts_added === 1 ? "has" : "have"} no tabs here yet.</li>
      </ul>
      {#if nothing}<p class="note">This computer already has everything in the file.</p>{/if}
    {/if}

    {#if error}<p class="error">{error}</p>{/if}

    <div class="actions">
      {#if done}
        <button class="primary" onclick={onclose}>Done</button>
      {:else}
        <button onclick={onclose} disabled={running}>Cancel</button>
        <button class="primary" onclick={run} disabled={running || nothing}>{running ? "Importing…" : "Import"}</button>
      {/if}
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
    width: min(560px, calc(100% - 32px));
    max-height: calc(100% - 32px);
    overflow-y: auto;
    padding: 18px 20px 16px;
    color: var(--jojo-fg2);
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
  .hint,
  .note {
    margin: 0 0 10px;
    color: var(--jojo-muted);
    font-size: 12px;
  }
  code {
    font-family: var(--jojo-font-mono);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }
  .locations li {
    padding: 6px 0;
    border-bottom: var(--jojo-bw) solid var(--jojo-line);
  }
  .locations li:last-child {
    border-bottom: none;
  }
  .line {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }
  .label {
    color: var(--jojo-fg);
  }
  .address {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--jojo-muted);
    font-family: var(--jojo-font-mono);
    font-size: 11.5px;
  }
  .tag {
    color: var(--jojo-done);
    font-size: 11.5px;
  }
  .tag.muted {
    color: var(--jojo-muted);
  }
  .password {
    display: grid;
    grid-template-columns: 70px 1fr;
    align-items: center;
    gap: 8px;
    margin: 6px 0 4px;
  }
  .password + .note {
    margin: 0 0 4px 78px;
  }
  input {
    padding: 5px 8px;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    background: var(--jojo-pane);
    color: var(--jojo-fg);
    font: inherit;
  }
  .warn {
    margin: 6px 0 0;
    color: var(--jojo-needs-you);
    font-size: 12px;
  }
  .summary li,
  .notes li {
    margin-bottom: 6px;
    font-size: 12.5px;
  }
  .notes li {
    color: var(--jojo-needs-you);
  }
  .error {
    margin: 10px 0 0;
    color: var(--jojo-error);
    font-size: 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
  }
  .actions button {
    padding: 5px 12px;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    color: var(--jojo-fg2);
  }
  .actions .primary {
    border-color: var(--jojo-accent);
    background: var(--jojo-accent);
    color: var(--jojo-bg);
  }
  .actions button:disabled {
    opacity: 0.5;
  }
</style>
