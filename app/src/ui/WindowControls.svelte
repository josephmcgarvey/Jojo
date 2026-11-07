<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  const win = getCurrentWindow();
</script>

<div class="controls">
  <button aria-label="Minimize" onclick={() => win.minimize()}>&#xE921;</button>
  <button aria-label="Maximize" onclick={() => win.toggleMaximize()}>&#xE922;</button>
  <button aria-label="Close" class="close" onclick={() => win.close()}>&#xE8BB;</button>
</div>

<style>
  .controls {
    display: flex;
    align-self: stretch;
  }
  button {
    width: 46px;
    font-family: "Segoe Fluent Icons", "Segoe MDL2 Assets";
    font-size: 10px;
    color: var(--jojo-fg2);
  }
  button:hover {
    background: var(--jojo-sel);
  }
  .close:hover {
    background: var(--jojo-danger);
    color: var(--jojo-fg);
  }
</style>
