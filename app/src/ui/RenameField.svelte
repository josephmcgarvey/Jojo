<script lang="ts">
  let { value, label, onCommit, onCancel }: { value: string; label: string; onCommit: (value: string) => void; onCancel: () => void } = $props();
  let input: HTMLInputElement;
  let finished = false;

  $effect(() => {
    input.focus();
    input.select();
  });

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

<input bind:this={input} class="rename" aria-label={label} {value} {onkeydown} onblur={() => finish(true)} />

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
