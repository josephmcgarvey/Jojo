<script lang="ts">
  // Spec 9.6, Ctrl+Shift+P: fuzzy search over sessions, projects and actions,
  // followed by the lines of every session's output that contain the query
  // ("In output", searchOutput in app.svelte.ts). Up and Down move the
  // highlight, Enter runs it, Esc closes.
  import { LOCAL_LABEL } from "../lib/platform";
  import { onMount, tick } from "svelte";
  import { terminalControls } from "../lib/terminal/registry";
  import {
    app,
    confirmRestartLocalDaemon,
    displayLabel,
    focusedSessionId,
    focusPane,
    jumpToMatch,
    newSession,
    searchOutput,
    rehydrateFocusedPane,
    selectProject,
  } from "../lib/state/app.svelte";
  import { orderedSessions } from "../lib/state/views";
  import { openDialog, ui } from "../lib/state/ui.svelte";
  import { fuzzyFilter } from "../lib/switcher/fuzzy";
  import { highlightParts } from "../lib/switcher/highlight";
  import { stateTooltip } from "../lib/state/toasts";
  import { LOCAL_MACHINE } from "../lib/wire/bridge";
  import { insertSnippet, snippets, snippetTarget } from "../lib/state/snippets.svelte";
  import { appliesTo } from "../lib/snippets/library";

  let { onclose }: { onclose: () => void } = $props();

  interface Item {
    /** Unique among the items: `session:<id>`, `project:<machine>:<id>`, `action:<label>`, `snippet:<id>` or `output:<n>`. */
    key: string;
    kind: "Session" | "Project" | "Action" | "Snippet" | "Output";
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
    // Snippets for the focused terminal; choosing one inserts it without pressing Enter.
    const target = snippetTarget();
    if (target && terminalControls(focusedSessionId())) {
      for (const snippet of snippets.list.filter((s) => appliesTo(s.scope, target))) {
        list.push({ key: `snippet:${snippet.id}`, kind: "Snippet", label: snippet.name, detail: snippet.command.replace(/\s*\n\s*/g, " ⏎ "), run: () => insertSnippet(snippet.id, false) });
      }
    }
    const project = app.selectedProjectId;
    const actions: Omit<Item, "key" | "kind">[] = [
      { label: "New terminal", detail: "Ctrl+Shift+N", run: () => project && newSession(project, "shell") },
      { label: "New Claude session", detail: "Ctrl+Shift+T", run: () => project && newSession(project, "claude") },
      { label: "Add remote location…", detail: "Linux server over SSH", run: () => openDialog({ kind: "add-machine" }) },
      { label: "Settings…", detail: "", run: () => openDialog({ kind: "settings" }) },
      { label: `Restart daemon (${LOCAL_LABEL})…`, detail: `Ends ${LOCAL_LABEL}'s sessions`, run: () => void confirmRestartLocalDaemon() },
      { label: "Debug ▸ Rehydrate pane", detail: "New snapshot for the focused pane", run: rehydrateFocusedPane },
    ];
    if (app.machines.some((m) => m.id === LOCAL_MACHINE)) {
      list.push(...actions.map((action) => ({ ...action, key: `action:${action.label}`, kind: "Action" as const })));
    }
    return list;
  });

  const fuzzy = $derived(fuzzyFilter(items, query, (item) => `${item.label} ${item.detail}`).slice(0, 50));

  // Search every session's output 200 ms after the last keystroke.
  $effect(() => {
    const q = query;
    const timer = setTimeout(() => searchOutput(q), 200);
    return () => clearTimeout(timer);
  });
  const search = $derived(app.outputSearch && app.outputSearch.query === query.trim() ? app.outputSearch : null);
  const outputItems = $derived(
    (search?.hits ?? []).map(({ machine, hit }, n): Item => {
      const session = app.model.sessions[hit.session_id];
      return {
        key: `output:${n}`,
        kind: "Output",
        label: session ? displayLabel(session) : "session",
        detail: hit.text,
        run: () => jumpToMatch(hit.session_id, search!.query),
        // The location is shown next to the session name.
        location: machineLabel(machine),
      } as Item & { location: string };
    }),
  );
  const matches = $derived([...fuzzy, ...outputItems]);
  const outdatedNames = $derived((search?.outdated ?? []).map(machineLabel));

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
    // A session or output item focuses its own pane; otherwise keys return
    // to the focused pane's terminal instead of staying on the page.
    if (item.kind !== "Session" && item.kind !== "Output" && item.kind !== "Snippet") void tick().then(refocusTerminal);
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
    <input bind:this={input} bind:value={query} onkeydown={onKeydown} placeholder="Jump to a session, project or action, or search every session's output" spellcheck="false" />
    <ul role="listbox">
      {#each matches as item, i (item.key)}
        {#if item.kind === "Output" && (i === 0 || matches[i - 1].kind !== "Output")}
          <li class="heading" role="presentation">In output</li>
        {/if}
        <li role="option" aria-selected={i === highlight}>
          <button class:on={i === highlight} class:output={item.kind === "Output"} onmouseenter={() => (highlight = i)} onclick={() => choose(item)}>
            {#if item.kind === "Output"}
              <span class="label">{item.label} <small>{(item as Item & { location: string }).location}</small></span>
              <span class="line">{#each highlightParts(item.detail, search?.query ?? "") as part, p (p)}{#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}{/each}</span>
            {:else}
              <span class="kind">{item.kind}</span>
              <span class="label">{item.label}</span>
              <span class="detail">{item.detail}</span>
            {/if}
          </button>
        </li>
      {:else}
        {#if !search?.pending.length}<li class="empty">No matches</li>{/if}
      {/each}
      {#if search && search.pending.length > 0}
        <li class="note">Searching output on {search.pending.length} location{search.pending.length === 1 ? "" : "s"}…</li>
      {:else if search && search.hits.length === 0 && query.trim().length >= 2}
        <li class="note">No output contains “{search.query}”.</li>
      {/if}
      {#if outdatedNames.length > 0}
        <li class="note">Not searched: {outdatedNames.join(", ")} (jojod is older than the app; restart it or add the remote location again to update it).</li>
      {/if}
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
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius);
    box-shadow: var(--jojo-shadow);
    overflow: hidden;
    outline: none;
  }
  input {
    padding: 10px 14px;
    border: none;
    border-bottom: var(--jojo-bw) solid var(--jojo-line);
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
  .empty,
  .note {
    padding: 8px 10px;
    color: var(--jojo-muted);
    font-size: 12px;
  }
  /* "In output": a heading, then one row per matching line: the session
     and its location above the line, the match marked. */
  .heading {
    margin-top: 6px;
    padding: 6px 10px 2px;
    border-top: var(--jojo-bw) solid var(--jojo-line);
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--jojo-muted);
  }
  button.output {
    grid-template-columns: 1fr;
    gap: 2px;
  }
  button.output .label {
    font-size: 12px;
  }
  button.output small {
    margin-left: 6px;
    color: var(--jojo-muted);
    font-size: 11px;
  }
  .line {
    font-family: var(--jojo-font-mono);
    font-size: 12px;
    color: var(--jojo-fg2);
    white-space: pre;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  mark {
    padding: 0 1px;
    border-radius: 2px;
    background: color-mix(in srgb, var(--jojo-needs-you) 35%, transparent);
    color: var(--jojo-fg);
  }
</style>
