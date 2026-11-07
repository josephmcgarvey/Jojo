<script lang="ts">
  import type { ITheme } from "@xterm/xterm";
  import { layoutRects, type DividerRect } from "../lib/layout/geometry";
  import { resizeSplit } from "../lib/layout/tree";
  import { app, currentTab, setTabRoot } from "../lib/state/app.svelte";
  import { paneKey } from "../lib/terminal/pane";
  import PaneView from "./PaneView.svelte";

  let { terminalTheme }: { terminalTheme: ITheme } = $props();
  let area: HTMLDivElement;
  const tab = $derived(currentTab());
  const rects = $derived(tab ? layoutRects(tab.root, tab.zoomed) : { panes: [], dividers: [] });
  const percent = (fraction: number) => `${fraction * 100}%`;

  /**
   * Drags a divider (spec 9.3 rule 3). Every move recomputes the sizes from
   * the tree as it was when the drag started plus the total pointer
   * movement, so clamping at MIN_SHARE never lets the divider drift away
   * from the pointer.
   */
  function startDrag(event: PointerEvent, divider: DividerRect): void {
    if (!tab || event.button !== 0) return;
    event.preventDefault();
    const tabId = tab.id;
    const base = $state.snapshot(tab.root);
    const handle = event.currentTarget as HTMLElement;
    handle.setPointerCapture(event.pointerId);
    const box = area.getBoundingClientRect();
    const extentPx = (divider.direction === "row" ? box.width : box.height) * divider.extent;
    const start = divider.direction === "row" ? event.clientX : event.clientY;
    const move = (e: PointerEvent) => {
      if (extentPx <= 0) return;
      const now = divider.direction === "row" ? e.clientX : e.clientY;
      setTabRoot(tabId, resizeSplit(base, divider.path, divider.index, (now - start) / extentPx));
    };
    const end = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", end);
      handle.removeEventListener("pointercancel", end);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", end);
    handle.addEventListener("pointercancel", end);
  }
</script>

<div class="area" bind:this={area}>
  {#each rects.panes as rect (rect.sessionId)}
    {@const session = app.model.sessions[rect.sessionId]}
    {#if session}
      <div class="slot" style:left={percent(rect.x)} style:top={percent(rect.y)} style:width={percent(rect.w)} style:height={percent(rect.h)}>
        {#key paneKey(session)}
          <PaneView {session} {terminalTheme} focused={tab?.focused === session.id} />
        {/key}
      </div>
    {/if}
  {/each}
  {#each rects.dividers as divider (`${divider.path.join(".")}/${divider.index}`)}
    <!-- Dividers are pointer-only; Layout ▸ presets and Equalize sizes in the pane menu cover the keyboard. -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div
      class="divider {divider.direction}"
      role="separator"
      aria-orientation={divider.direction === "row" ? "vertical" : "horizontal"}
      style:left={percent(divider.x)}
      style:top={percent(divider.y)}
      style:width={divider.direction === "row" ? null : percent(divider.length)}
      style:height={divider.direction === "row" ? percent(divider.length) : null}
      onpointerdown={(e) => startDrag(e, divider)}
    ></div>
  {/each}
</div>

<style>
  .area {
    position: absolute;
    inset: 0;
    padding: 3px;
  }
  .slot {
    position: absolute;
    padding: 3px;
    min-width: 0;
    min-height: 0;
  }
  .divider {
    position: absolute;
    z-index: 1;
  }
  .divider.row {
    width: 6px;
    margin-left: -3px;
    cursor: col-resize;
  }
  .divider.column {
    height: 6px;
    margin-top: -3px;
    cursor: row-resize;
  }
  .divider:hover {
    background: var(--jojo-line);
  }
</style>
