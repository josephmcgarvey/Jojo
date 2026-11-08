<script lang="ts">
  import { answerConfirm, dialog } from "../lib/state/dialog.svelte";

  let confirmButton = $state<HTMLButtonElement | undefined>();

  // The confirm button takes focus when the dialog opens, so Enter confirms
  // and Escape (handled below) cancels without touching the mouse.
  $effect(() => {
    if (dialog.confirm) confirmButton?.focus();
  });

  function onkeydown(event: KeyboardEvent): void {
    if (event.key === "Escape") {
      event.preventDefault();
      answerConfirm(false);
    }
  }
</script>

{#if dialog.confirm}
  <!-- Clicking the scrim outside the dialog cancels; the dialog itself is keyboard operable. -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="scrim" onpointerdown={(e) => e.target === e.currentTarget && answerConfirm(false)}>
    <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title" tabindex="-1" {onkeydown}>
      <h2 id="confirm-title">{dialog.confirm.title}</h2>
      <p>{dialog.confirm.message}</p>
      {#if dialog.confirm.details.length > 0}
        <ul>
          {#each dialog.confirm.details as line}
            <li>{line}</li>
          {/each}
        </ul>
      {/if}
      <div class="actions">
        <button onclick={() => answerConfirm(false)}>Cancel</button>
        <button class="primary" bind:this={confirmButton} onclick={() => answerConfirm(true)}>{dialog.confirm.confirmLabel}</button>
      </div>
    </div>
  </div>
{/if}

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
    width: min(420px, calc(100% - 32px));
    padding: 18px 20px 16px;
    background: var(--jojo-menu);
    border: 1px solid var(--jojo-line);
    border-radius: var(--jojo-radius);
    box-shadow: var(--jojo-shadow);
    outline: none;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 14px;
    font-weight: 600;
    color: var(--jojo-fg);
  }
  p {
    margin: 0 0 8px;
    color: var(--jojo-fg2);
  }
  ul {
    margin: 0 0 8px;
    padding-left: 18px;
    color: var(--jojo-fg2);
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
    border-color: var(--jojo-danger);
    background: var(--jojo-danger);
    color: var(--jojo-fg);
  }
</style>
