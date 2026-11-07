<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { toPhysicalRect } from "../lib/window/snap";

  const win = getCurrentWindow();
  let maximize: HTMLButtonElement;
  let hovered = $state(false);

  onMount(() => {
    // The snap-layout input sink (app/src-tauri/src/snap.rs) sits over this
    // button and receives its mouse input. Its position is sent whenever it
    // can change, and its hover state comes back as an event.
    const report = () => {
      const rect = toPhysicalRect(maximize.getBoundingClientRect(), window.devicePixelRatio);
      void invoke("set_maximize_button_rect", { ...rect }).catch(() => {});
    };
    const observer = new ResizeObserver(report);
    observer.observe(maximize);
    window.addEventListener("resize", report);
    report();
    const unlisten = listen<boolean>("maximize-hover", (event) => {
      hovered = event.payload;
    });
    return () => {
      observer.disconnect();
      window.removeEventListener("resize", report);
      void unlisten.then((stop) => stop());
    };
  });
</script>

<div class="controls">
  <button aria-label="Minimize" onclick={() => win.minimize()}>&#xE921;</button>
  <!-- Mouse input goes to the input sink above this button; keyboard activation (Tab, then Enter) reaches the button. -->
  <button bind:this={maximize} class:hover={hovered} aria-label="Maximize" onclick={() => win.toggleMaximize()}>&#xE922;</button>
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
  button:hover,
  button.hover {
    background: var(--jojo-sel);
  }
  .close:hover {
    background: var(--jojo-danger);
    color: var(--jojo-fg);
  }
</style>
