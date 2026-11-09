<script lang="ts">
  // The Heroicons solid "plus-circle" (MIT license, heroicons.com): a filled
  // circle with the plus knocked out, drawn in currentColor. Used for every
  // "add" button (design pass, section 2.2).
  let { size = 18 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
  <path
    fill-rule="evenodd"
    clip-rule="evenodd"
    d="M12 2.25c-5.385 0-9.75 4.365-9.75 9.75s4.365 9.75 9.75 9.75 9.75-4.365 9.75-9.75S17.385 2.25 12 2.25ZM12.75 9a.75.75 0 0 0-1.5 0v2.25H9a.75.75 0 0 0 0 1.5h2.25V15a.75.75 0 0 0 1.5 0v-2.25H15a.75.75 0 0 0 0-1.5h-2.25V9Z"
  />
</svg>

<style>
  svg {
    display: block;
    flex: none;
  }
</style>
