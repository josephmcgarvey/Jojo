<script lang="ts">
  // The Flag for later mark (lib/state/flags.ts): a small pennant in the
  // theme's accent color on a flagged session's sidebar row, pane name bar
  // and tab card. It is not a button; the session and pane menus remove it.
  let { size = 11 }: { size?: number } = $props();
</script>

<svg class="flag" width={size} height={size} viewBox="0 0 16 16" role="img" aria-label="Flagged for later">
  <title>Flagged for later (right-click to remove the flag)</title>
  <path d="M3.5 14.5V2" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" />
  <path d="M3.5 2.5h8.2l-2 3.25 2 3.25H3.5z" fill="currentColor" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" />
</svg>

<style>
  .flag {
    display: inline-block;
    flex: none;
    color: var(--jojo-accent);
  }
</style>
