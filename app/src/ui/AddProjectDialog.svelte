<script lang="ts">
  // Adds a project on a remote machine: the folder is a path on that
  // machine, so the Windows folder picker cannot choose it.
  import { addRemoteProject, app, listDir } from "../lib/state/app.svelte";
  import { folderSuggestions, splitTypedPath } from "../lib/state/pathSuggest";
  import { errorText } from "../lib/errors";

  let { machineId, onclose }: { machineId: string; onclose: () => void } = $props();

  const machine = $derived(app.machines.find((m) => m.id === machineId));
  let path = $state("~/");
  let error = $state<string | null>(null);

  // Spec 9.2: suggestions from ListDir for the folder being typed in, asked
  // 150 ms after the last keystroke.
  const typed = $derived(splitTypedPath(path));
  // A string, so the effect below runs when the folder changes, not on
  // every keystroke within it.
  const typedDir = $derived(typed.dir);
  $effect(() => {
    const dir = typedDir;
    const timer = setTimeout(() => listDir(machineId, dir), 150);
    return () => clearTimeout(timer);
  });
  const suggestions = $derived(
    app.dirEntries && app.dirEntries.machine === machineId && app.dirEntries.path === typed.dir
      ? folderSuggestions(typed.dir, typed.prefix, app.dirEntries.entries)
      : [],
  );

  async function add(): Promise<void> {
    try {
      await addRemoteProject(machineId, path);
      onclose();
    } catch (err) {
      error = errorText(err);
    }
  }

  let input: HTMLInputElement;

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") onclose();
    // Enter on a focused suggestion chooses it (its own click); only Enter
    // in the path field adds the project.
    if (event.key === "Enter" && event.target === input && path.trim()) void add();
  }

  function choose(suggestion: string): void {
    path = suggestion;
    input.focus();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="scrim" onkeydown={onKeydown}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="add-project-title">
    <h2 id="add-project-title">Add project on {machine?.label ?? "server"}</h2>
    <p class="hint">The folder's path on the server, for example <code>/home/you/app</code>. Sessions in this project start there.</p>
    <!-- svelte-ignore a11y_autofocus -->
    <input bind:this={input} bind:value={path} autofocus spellcheck="false" />
    {#if suggestions.length > 0}
      <ul class="suggestions">
        {#each suggestions as suggestion (suggestion)}
          <li><button onclick={() => choose(suggestion)}>{suggestion}</button></li>
        {/each}
      </ul>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}
    <div class="actions">
      <button onclick={onclose}>Cancel</button>
      <button class="primary" onclick={add} disabled={!path.trim()}>Add project</button>
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
    width: min(460px, calc(100% - 32px));
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
  .hint {
    margin: 0 0 10px;
    color: var(--jojo-muted);
    font-size: 12px;
  }
  code,
  input {
    font-family: var(--jojo-font-mono);
  }
  input {
    width: 100%;
    box-sizing: border-box;
    padding: 5px 8px;
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    background: var(--jojo-pane);
    color: var(--jojo-fg);
  }
  .suggestions {
    margin: 4px 0 0;
    padding: 4px;
    max-height: 180px;
    overflow-y: auto;
    list-style: none;
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
  }
  .suggestions button {
    width: 100%;
    padding: 3px 6px;
    text-align: left;
    color: var(--jojo-fg2);
    font-family: var(--jojo-font-mono);
    border-radius: var(--jojo-radius-sm);
  }
  .suggestions button:hover {
    background: var(--jojo-sel);
    color: var(--jojo-fg);
  }
  .error {
    margin: 8px 0 0;
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
    border: 1px solid var(--jojo-line);
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
