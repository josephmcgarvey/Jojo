<script lang="ts">
  // Creates or edits a snippet: its name, command, folder and where it is
  // listed (every location, the focused terminal's location, or its
  // project). Opened from the Snippets list (New snippet, Edit), which it
  // returns to, or from a pane's Save as snippet, which it closes back to
  // the terminal.
  import { untrack } from "svelte";
  import type { SnippetScope } from "../lib/backend/SnippetScope";
  import { app } from "../lib/state/app.svelte";
  import { addSnippet, deleteSnippet, snippets, snippetTarget, updateSnippet } from "../lib/state/snippets.svelte";
  import { openDialog } from "../lib/state/ui.svelte";
  import { folderNames, nameFromText } from "../lib/snippets/library";

  let { snippetId, command, onclose }: { snippetId: string | null; command: string; onclose: () => void } = $props();

  // The dialog is created each time it opens, so its props are read once.
  const startCommand = untrack(() => command);
  const existing = untrack(() => (snippetId ? snippets.list.find((s) => s.id === snippetId) : undefined));
  /** Saved from a terminal selection: Cancel and Save return to the terminal, not to the Snippets list. */
  const fromSelection = !existing && startCommand !== "";
  const target = snippetTarget();

  let name = $state(existing?.name ?? nameFromText(startCommand));
  let text = $state(existing?.command ?? startCommand);
  let folder = $state(existing?.folder ?? "");

  interface ScopeOption {
    value: string;
    label: string;
    scope: SnippetScope;
  }
  // The choices: every location, plus the location and project of the
  // focused terminal and of the snippet's current scope (which may differ
  // when editing a snippet listed with "Show … for other locations").
  const scopeOptions: ScopeOption[] = (() => {
    const options: ScopeOption[] = [{ value: "all", label: "Every location and project", scope: { kind: "all" } }];
    const machineIds = new Set<string>();
    const projectIds = new Set<string>();
    if (target) {
      machineIds.add(target.machineId);
      projectIds.add(target.projectId);
    }
    if (existing?.scope.kind === "machine") machineIds.add(existing.scope.machine_id);
    if (existing?.scope.kind === "project") projectIds.add(existing.scope.project_id);
    for (const id of machineIds) {
      const label = app.machines.find((m) => m.id === id)?.label ?? "a removed location";
      options.push({ value: `machine:${id}`, label: `Only terminals on ${label}`, scope: { kind: "machine", machine_id: id } });
    }
    for (const id of projectIds) {
      const label = app.model.projects[id]?.name ?? "a removed project";
      options.push({ value: `project:${id}`, label: `Only terminals in the ${label} project`, scope: { kind: "project", project_id: id } });
    }
    return options;
  })();
  const scopeValue = (scope: SnippetScope) => (scope.kind === "all" ? "all" : scope.kind === "machine" ? `machine:${scope.machine_id}` : `project:${scope.project_id}`);
  let scope = $state(existing ? scopeValue(existing.scope) : "all");

  const folders = folderNames(snippets.list);
  const valid = $derived(name.trim() !== "" && text.trim() !== "");

  function finish(): void {
    if (fromSelection) onclose();
    else openDialog({ kind: "snippets" });
  }

  function save(): void {
    if (!valid) return;
    const fields = { name: name.trim(), command: text, folder: folder.trim(), scope: scopeOptions.find((o) => o.value === scope)?.scope ?? { kind: "all" as const } };
    if (existing) updateSnippet(existing.id, fields);
    else addSnippet(fields);
    finish();
  }

  async function remove(): Promise<void> {
    if (!existing) return;
    await deleteSnippet(existing.id);
    if (!snippets.list.some((s) => s.id === existing.id)) finish();
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      finish();
    } else if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) {
      event.preventDefault();
      save();
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="scrim" onkeydown={onKeydown}>
  <div class="dialog" role="dialog" aria-modal="true" aria-labelledby="snippet-edit-title">
    <h2 id="snippet-edit-title">{existing ? "Edit snippet" : "New snippet"}</h2>
    <label class="field">
      <span>Name</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input bind:value={name} autofocus spellcheck="false" placeholder="Deploy to staging" />
    </label>
    <label class="field">
      <span>Command</span>
      <textarea bind:value={text} rows="5" spellcheck="false" placeholder="git pull && php artisan migrate"></textarea>
      <small>Several lines are inserted as one paste. Inserting never presses Enter; Run does.</small>
    </label>
    <div class="pair">
      <label class="field">
        <span>Folder</span>
        <input bind:value={folder} list="snippet-folders" spellcheck="false" placeholder="None" />
        <datalist id="snippet-folders">
          {#each folders as f (f)}<option value={f}></option>{/each}
        </datalist>
      </label>
      <label class="field">
        <span>Show for</span>
        <select bind:value={scope}>
          {#each scopeOptions as option (option.value)}<option value={option.value}>{option.label}</option>{/each}
        </select>
      </label>
    </div>
    <div class="actions">
      {#if existing}<button class="delete" onclick={() => void remove()}>Delete</button>{/if}
      <span class="grow"></span>
      <button onclick={finish}>Cancel</button>
      <button class="primary" onclick={save} disabled={!valid}>Save</button>
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
    width: min(520px, calc(100% - 32px));
    display: grid;
    gap: 12px;
    padding: 18px 20px 16px;
    background: var(--jojo-menu);
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius);
    box-shadow: var(--jojo-shadow);
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
    min-width: 0;
  }
  .field > span {
    color: var(--jojo-fg2);
    font-size: 12px;
    font-weight: 500;
  }
  .field small {
    color: var(--jojo-muted);
    font-size: 11.5px;
  }
  .pair {
    display: grid;
    grid-template-columns: 1fr 1.4fr;
    gap: 12px;
  }
  input,
  textarea,
  select {
    width: 100%;
    box-sizing: border-box;
    padding: 5px 8px;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    background: var(--jojo-pane);
    color: var(--jojo-fg);
    font: inherit;
  }
  textarea {
    resize: vertical;
    font-family: var(--jojo-font-mono);
    font-size: 12px;
    line-height: 1.45;
  }
  input:focus,
  textarea:focus,
  select:focus {
    outline: none;
    border-color: var(--jojo-focus-border);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 2px;
  }
  .grow {
    flex: 1;
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
  .actions .delete {
    color: var(--jojo-error);
  }
  .actions button:disabled {
    opacity: 0.5;
  }
</style>
