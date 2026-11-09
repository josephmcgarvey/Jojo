<script lang="ts">
  // Spec 9.6, Ctrl+Shift+P: fuzzy search over sessions, projects and actions.
  // Up and Down move the highlight, Enter runs it, Esc closes.
  import { onMount, tick } from "svelte";
  import { terminalControls } from "../lib/terminal/registry";
  import {
    app,
    confirmRestartLocalDaemon,
    displayLabel,
    focusedSessionId,
    focusPane,
    newSession,
    rehydrateFocusedPane,
    selectProject,
  } from "../lib/state/app.svelte";
  import { orderedSessions } from "../lib/state/views";
  import { openDialog, ui } from "../lib/state/ui.svelte";
  import { fuzzyFilter } from "../lib/switcher/fuzzy";
  import { stateTooltip } from "../lib/state/toasts";
  import { LOCAL_MACHINE } from "../lib/wire/bridge";

  let { onclose }: { onclose: () => void } = $props();

  interface Item {
    /** Unique among the items: `session:<id>`, `project:<machine>:<id>` or `action:<label>`. */
    key: string;
    kind: "Session" | "Project" | "Action";
    label: string;
    detail: string;
    run: () => void;
  }

  let query = $state("");
  let highlight = $state(0);
  let input: HTMLInputElement;

  const machineLabel = (id: string) => app.machines.find((m) => m.id === id)?.label ?? id;

  const items = $derived.by((): Item[] => {
    const list: Item[] = [];
    for (const { session, machineId } of orderedSessions(app.machines, app.models)) {
      const project = app.model.projects[session.project_id];
      list.push({
        key: `session:${session.id}`,
        kind: "Session",
        label: displayLabel(session),
        detail: `${project?.name ?? ""} · ${machineLabel(machineId)} · ${stateTooltip(session)}`,
        run: () => focusPane(session.id),
      });
    }
    for (const machine of app.machines) {
      for (const project of Object.values(app.models[machine.id]?.projects ?? {})) {
        list.push({ key: `project:${machine.id}:${project.id}`, kind: "Project", label: project.name, detail: `${machine.label} · ${project.path}`, run: () => selectProject(project.id) });
      }
    }
    const project = app.selectedProjectId;
    const actions: Omit<Item, "key" | "kind">[] = [
      { label: "New terminal", detail: "Ctrl+Shift+N", run: () => project && newSession(project, "shell") },
      { label: "New Claude session", detail: "Ctrl+Shift+T", run: () => project && newSession(project, "claude") },
      { label: "Add machine…", detail: "Linux server over SSH", run: () => openDialog({ kind: "add-machine" }) },
      { label: "Settings…", detail: "", run: () => openDialog({ kind: "settings" }) },
      { label: "Restart daemon (This PC)…", detail: "Ends This PC's sessions", run: () => void confirmRestartLocalDaemon() },
      { label: "Debug ▸ Rehydrate pane", detail: "New snapshot for the focused pane", run: rehydrateFocusedPane },
    ];
    if (app.machines.some((m) => m.id === LOCAL_MACHINE)) {
      list.push(...actions.map((action) => ({ ...action, key: `action:${action.label}`, kind: "Action" as const })));
    }
    return list;
  });

  const matches = $derived(fuzzyFilter(items, query, (item) => `${item.label} ${item.detail}`).slice(0, 50));

  $effect(() => {
    // A new query starts at the best match.
    void query;
    highlight = 0;
  });

  onMount(() => input.focus());

  function choose(item: Item | undefined): void {
    if (!item) return;
    // Close first: an action that opens another dialog replaces this one.
    onclose();
    item.run();
    // A session item focuses its own pane; otherwise keys return to the
    // focused pane's terminal instead of staying on the page.
    if (item.kind !== "Session") void tick().then(refocusTerminal);
  }

  function close(): void {
    onclose();
    void tick().then(refocusTerminal);
  }

  function refocusTerminal(): void {
    if (!ui.dialog && !ui.menu) terminalControls(focusedSessionId())?.focus();
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      close();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      highlight = Math.max(0, Math.min(matches.length - 1, highlight + 1));
    } else if (event.key === "ArrowUp") {
      event.preventDefault();
      highlight = Math.max(0, highlight - 1);
    } else if (event.key === "Enter") {
      event.preventDefault();
      choose(matches[highlight]);
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<!-- svelte-ignore a11y_click_events_have_key_events -->
<div class="scrim" onclick={close}>
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <div class="switcher" role="dialog" aria-modal="true" aria-label="Switcher" tabindex="-1" onclick={(e) => e.stopPropagation()}>
    <input bind:this={input} bind:value={query} onkeydown={onKeydown} placeholder="Jump to a session, project or action" spellcheck="false" />
    <ul role="listbox">
      {#each matches as item, i (item.key)}
        <li role="option" aria-selected={i === highlight}>
          <button class:on={i === highlight} onmouseenter={() => (highlight = i)} onclick={() => choose(item)}>
            <span class="kind">{item.kind}</span>
            <span class="label">{item.label}</span>
            <span class="detail">{item.detail}</span>
          </button>
        </li>
      {:else}
        <li class="empty">No matches</li>
      {/each}
    </ul>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: flex;
    justify-content: center;
    padding-top: 12vh;
    background: var(--jojo-scrim);
  }
  .switcher {
    width: min(600px, calc(100% - 32px));
    max-height: 60vh;
    display: flex;
    flex-direction: column;
    background: var(--jojo-menu);
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius);
    box-shadow: var(--jojo-shadow);
    overflow: hidden;
    outline: none;
  }
  input {
    padding: 10px 14px;
    border: none;
    border-bottom: 1px solid var(--jojo-line);
    background: transparent;
    color: var(--jojo-fg);
    font: inherit;
    font-size: 14px;
    outline: none;
  }
  ul {
    margin: 0;
    padding: 4px;
    list-style: none;
    overflow-y: auto;
  }
  button {
    width: 100%;
    display: grid;
    grid-template-columns: 64px minmax(0, auto) 1fr;
    align-items: baseline;
    gap: 10px;
    padding: 6px 10px;
    border-radius: var(--jojo-radius-sm);
    text-align: left;
    color: var(--jojo-fg2);
  }
  button.on {
    background: var(--jojo-sel);
    color: var(--jojo-fg);
  }
  .kind {
    font-size: 10.5px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--jojo-muted);
  }
  .label {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .detail {
    color: var(--jojo-muted);
    font-size: 12px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .empty {
    padding: 8px 10px;
    color: var(--jojo-muted);
  }
</style>
