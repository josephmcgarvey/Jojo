<script lang="ts">
  import { activateSession, app, closeSession, newSession } from "../lib/state/app.svelte";
  import { sessionsForProject } from "../lib/state/views";
  import WindowControls from "./WindowControls.svelte";

  const tabs = $derived(app.selectedProjectId ? sessionsForProject(app.model, app.selectedProjectId) : []);
</script>

<div class="tabs">
  {#each tabs as session (session.id)}
    <div class="tab" class:on={app.activeSessionId === session.id}>
      <button class="title" onclick={() => activateSession(session)}>
        <span class="badge {session.state}"></span>{session.label}
      </button>
      <button class="close" aria-label="Close {session.label}" onclick={() => closeSession(session.id)}>×</button>
    </div>
  {/each}
  {#if app.selectedProjectId}
    <button class="new" title="New Claude session" onclick={() => newSession(app.selectedProjectId!, "claude")}>+</button>
  {/if}
  <div class="drag" data-tauri-drag-region></div>
  <WindowControls />
</div>

<style>
  .tabs {
    display: flex;
    align-items: flex-end;
    gap: 4px;
    height: 38px;
    padding: 6px 0 0 8px;
    background: var(--jojo-bar);
    border-bottom: 1px solid var(--jojo-line);
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 6px 0 12px;
    height: 31px;
    border-radius: var(--jojo-radius) var(--jojo-radius) 0 0;
    color: var(--jojo-muted);
  }
  .tab.on {
    background: var(--jojo-pane);
    color: var(--jojo-fg);
    border: 1px solid var(--jojo-line);
    border-bottom-color: transparent;
    margin-bottom: -1px;
  }
  .title {
    display: flex;
    align-items: center;
    gap: 7px;
    max-width: 220px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .close {
    padding: 0 4px;
    color: var(--jojo-muted);
  }
  .close:hover {
    color: var(--jojo-fg);
  }
  .new {
    height: 31px;
    padding: 0 10px;
    color: var(--jojo-muted);
  }
  .drag {
    flex: 1;
    align-self: stretch;
  }
  .badge {
    width: 11px;
    height: 11px;
    flex: none;
    border-radius: 50%;
    border: 2px solid var(--jojo-idle);
  }
  .badge.done {
    border-color: var(--jojo-done);
    background: var(--jojo-done);
  }
  .badge.error {
    border-color: var(--jojo-error);
    background: var(--jojo-error);
  }
</style>
