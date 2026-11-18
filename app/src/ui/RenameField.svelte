<script lang="ts">
  import { onMount } from "svelte";
  import { ui } from "../lib/state/ui.svelte";

  let { value, label, onCommit, onCancel }: { value: string; label: string; onCommit: (value: string) => void; onCancel: () => void } = $props();
  let input: HTMLInputElement;
  let finished = false;

  // The field's text is set once, when it mounts, and is not bound to
  // `value`, so a re-render of the row never replaces what is being typed.
  // The text typed so far is kept in ui.renaming.draft: when the row is
  // rebuilt while the rename is open (its project or session list is
  // replaced), the new field shows the draft with the cursor at its end,
  // instead of the old name selected, where the next key would replace it.
  onMount(() => {
    const draft = ui.renaming?.draft;
    input.value = draft ?? value;
    input.focus();
    if (draft === undefined) input.select();
    else input.setSelectionRange(draft.length, draft.length);
  });

  function oninput(): void {
    if (ui.renaming) ui.renaming.draft = input.value;
  }

  /** Enter and blur commit; Escape cancels. `finished` stops the blur that follows Enter or Escape from acting twice. */
  function finish(commit: boolean): void {
    if (finished) return;
    finished = true;
    if (commit) onCommit(input.value);
    else onCancel();
  }

  function onkeydown(event: KeyboardEvent): void {
    // Keys typed here never reach the terminal or the app shortcuts.
    event.stopPropagation();
    if (event.key === "Enter") finish(true);
    else if (event.key === "Escape") finish(false);
  }
</script>

<input bind:this={input} class="rename" aria-label={label} {oninput} {onkeydown} onblur={() => finish(true)} />

<style>
  .rename {
    flex: 1;
    min-width: 0;
    padding: 1px 5px;
    font: inherit;
    color: var(--jojo-fg);
    background: var(--jojo-pane);
    border: 1px solid var(--jojo-accent);
    border-radius: 4px;
    outline: none;
  }
</style>
