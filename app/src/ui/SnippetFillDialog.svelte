<script lang="ts">
  // Asks for the values of a snippet's {{name}} placeholders before it is
  // inserted or run (insertSnippet in snippets.svelte.ts opens it). Each
  // field starts with the value last typed for that name, else the default
  // written in the command ({{name:default}}). The preview shows the command
  // as it will be typed. Enter does what the snippet was opened for (Insert
  // from a click, Run from Run or Ctrl+Enter); Ctrl+Enter (⌘Enter) always
  // runs; Esc returns to the terminal without typing anything.
  import { tick, untrack } from "svelte";
  import { isMac } from "../lib/platform";
  import { focusedSessionId } from "../lib/state/app.svelte";
  import { fillAndInsertSnippet, snippets } from "../lib/state/snippets.svelte";
  import { ui } from "../lib/state/ui.svelte";
  import { fillPlaceholders, placeholders, startingValue } from "../lib/snippets/library";
  import { terminalControls } from "../lib/terminal/registry";

  let { snippetId, run, onclose }: { snippetId: string; run: boolean; onclose: () => void } = $props();

  // The dialog is created each time it opens, so its props are read once.
  const snippet = untrack(() => snippets.list.find((s) => s.id === snippetId));
  const runByDefault = untrack(() => run);
  const fields = snippet ? placeholders(snippet.command) : [];
  let values = $state<Record<string, string>>(Object.fromEntries(fields.map((f) => [f.name, startingValue(f, snippet?.last_values ?? {})])));
  const preview = $derived(snippet ? fillPlaceholders(snippet.command, values) : "");

  function submit(asRun: boolean): void {
    if (!snippet) return;
    onclose();
    fillAndInsertSnippet(snippet.id, $state.snapshot(values), asRun);
  }

  function cancel(): void {
    onclose();
    void tick().then(() => {
      if (!ui.dialog && !ui.menu) terminalControls(focusedSessionId())?.focus();
    });
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      cancel();
    } else if (event.key === "Enter" && event.target instanceof HTMLInputElement) {
      event.preventDefault();
      submit((isMac ? event.metaKey : event.ctrlKey) || runByDefault);
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="scrim jojo-scrim" onkeydown={onKeydown}>
  <div class="dialog jojo-modal" role="dialog" aria-modal="true" aria-labelledby="snippet-fill-title">
    <h2 id="snippet-fill-title">{snippet?.name ?? "Snippet"}</h2>
    {#if snippet}
      {#each fields as field, i (field.name)}
        <label class="field">
          <span>{field.name}</span>
          {#if i === 0}
            <!-- svelte-ignore a11y_autofocus -->
            <input bind:value={values[field.name]} autofocus spellcheck="false" placeholder={field.fallback || field.name} />
          {:else}
            <input bind:value={values[field.name]} spellcheck="false" placeholder={field.fallback || field.name} />
          {/if}
        </label>
      {/each}
      <div class="preview" aria-label="Command to be typed">{preview}</div>
      <div class="actions">
        <span class="hint">Enter {runByDefault ? "runs" : "inserts"} · <kbd>{isMac ? "⌘" : "Ctrl+"}Enter</kbd> runs</span>
        <button onclick={cancel}>Cancel</button>
        <button class:primary={!runByDefault} onclick={() => submit(false)}>Insert</button>
        <button class:primary={runByDefault} onclick={() => submit(true)}>Run</button>
      </div>
    {:else}
      <p class="hint">This snippet no longer exists.</p>
      <div class="actions"><button onclick={cancel}>Close</button></div>
    {/if}
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
    width: min(480px, calc(100% - 32px));
    display: grid;
    gap: 10px;
    padding: 18px 20px 16px;
  }
  h2 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--jojo-fg);
  }
  .field {
    display: grid;
    gap: 4px;
  }
  .field > span {
    color: var(--jojo-fg2);
    font-family: var(--jojo-font-mono);
    font-size: 12px;
  }
  input {
    width: 100%;
    box-sizing: border-box;
    padding: 5px 8px;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    background: var(--jojo-pane);
    color: var(--jojo-fg);
    font-family: var(--jojo-font-mono);
    font-size: 12.5px;
  }
  input:focus {
    outline: none;
    border-color: var(--jojo-focus-border);
  }
  .preview {
    padding: 8px 10px;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    background: var(--jojo-pane);
    color: var(--jojo-fg2);
    font-family: var(--jojo-font-mono);
    font-size: 12px;
    white-space: pre-wrap;
    word-break: break-all;
    max-height: 160px;
    overflow-y: auto;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 2px;
  }
  .hint {
    flex: 1;
    margin: 0;
    color: var(--jojo-muted);
    font-size: 11.5px;
  }
  kbd {
    font-family: var(--jojo-font-mono);
    font-size: 11px;
    color: var(--jojo-fg2);
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
</style>
