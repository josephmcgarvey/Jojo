<script lang="ts">
  import { onMount } from "svelte";
  import { firstEnabled, moveHighlight, placeMenu, placeSubmenu, type Box } from "../lib/menu/navigation";
  import { isEnabled, type MenuEntry } from "../lib/menu/types";
  import Self from "./ContextMenu.svelte";

  let {
    entries,
    x = 0,
    y = 0,
    row = null,
    autofocus = true,
    onclose,
    onback = null,
  }: {
    entries: MenuEntry[];
    /** Pointer position, for the root menu. */
    x?: number;
    y?: number;
    /** The parent row's rectangle, for a submenu; null for the root menu. */
    row?: Box | null;
    /** Take keyboard focus on mount: the root menu, and submenus opened from the keyboard. */
    autofocus?: boolean;
    /** Closes the whole menu. */
    onclose: () => void;
    /** Closes this submenu and returns keyboard focus to its parent. */
    onback?: (() => void) | null;
  } = $props();

  let element: HTMLDivElement;
  let highlighted = $state(-1);
  let open = $state<{ index: number; row: Box; viaKeyboard: boolean } | null>(null);
  let left = $state(0);
  let top = $state(0);
  let placed = $state(false);

  onMount(() => {
    const size = { w: element.offsetWidth, h: element.offsetHeight };
    const viewport = { w: window.innerWidth, h: window.innerHeight };
    const at = row ? placeSubmenu(row, size, viewport) : placeMenu({ x, y }, size, viewport);
    left = at.x;
    top = at.y;
    placed = true;
    if (autofocus) {
      element.focus();
      if (row) highlighted = firstEnabled(entries);
    }
    if (row) return;
    // Only the root menu closes the menu on a click outside every menu
    // level, on window blur and on resize.
    const close = () => onclose();
    const onPointerDown = (event: PointerEvent) => {
      if (!(event.target instanceof Element && event.target.closest("[data-context-menu]"))) close();
    };
    window.addEventListener("pointerdown", onPointerDown, true);
    window.addEventListener("blur", close);
    window.addEventListener("resize", close);
    return () => {
      window.removeEventListener("pointerdown", onPointerDown, true);
      window.removeEventListener("blur", close);
      window.removeEventListener("resize", close);
    };
  });

  function itemsAt(index: number): MenuEntry[] {
    const entry = entries[index];
    return entry?.kind === "submenu" ? entry.items : [];
  }

  function openSubmenu(index: number, viaKeyboard: boolean): void {
    const entry = entries[index];
    if (entry?.kind !== "submenu" || !isEnabled(entry)) {
      open = null;
      return;
    }
    const rowElement = element.querySelector<HTMLElement>(`[data-index="${index}"]`);
    if (!rowElement) return;
    const r = rowElement.getBoundingClientRect();
    open = { index, row: { x: r.left, y: r.top, w: r.width, h: r.height }, viaKeyboard };
  }

  function activate(index: number, viaKeyboard: boolean): void {
    const entry = entries[index];
    if (!entry || !isEnabled(entry)) return;
    if (entry.kind === "submenu") {
      openSubmenu(index, viaKeyboard);
    } else if (entry.kind === "action") {
      onclose();
      entry.run();
    }
  }

  function closeSubmenu(): void {
    open = null;
    element.focus();
  }

  // Spec 9.4: arrow keys, Enter and Esc operate every menu.
  function onkeydown(event: KeyboardEvent): void {
    switch (event.key) {
      case "ArrowDown":
        highlighted = moveHighlight(entries, highlighted, 1);
        break;
      case "ArrowUp":
        highlighted = moveHighlight(entries, highlighted, -1);
        break;
      case "ArrowRight":
        if (highlighted >= 0) openSubmenu(highlighted, true);
        break;
      case "ArrowLeft":
        onback?.();
        break;
      case "Enter":
      case " ":
        if (highlighted >= 0) activate(highlighted, true);
        break;
      case "Escape":
        if (onback) onback();
        else onclose();
        break;
      default:
        return;
    }
    event.preventDefault();
    event.stopPropagation();
  }
</script>

<div bind:this={element} class="menu" class:placed data-context-menu role="menu" tabindex="-1" style:left="{left}px" style:top="{top}px" {onkeydown}>
  {#each entries as entry, index (index)}
    {#if entry.kind === "separator"}
      <div class="separator" role="separator"></div>
    {:else}
      {@const enabled = isEnabled(entry)}
      <!-- Rows are operated from the keyboard through the menu element's keydown handler; tabindex -1 keeps them out of the Tab order. -->
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="row"
        class:highlighted={highlighted === index}
        class:disabled={!enabled}
        role="menuitem"
        tabindex="-1"
        data-index={index}
        aria-disabled={!enabled}
        aria-haspopup={entry.kind === "submenu" ? "menu" : undefined}
        title={entry.disabledReason}
        onpointerenter={() => {
          highlighted = enabled ? index : -1;
          if (entry.kind === "submenu") openSubmenu(index, false);
          else open = null;
        }}
        onclick={() => activate(index, false)}
      >
        <span class="label">{entry.label}</span>
        {#if entry.kind === "action" && entry.shortcut}<span class="shortcut">{entry.shortcut}</span>{/if}
        {#if entry.kind === "submenu"}<span class="arrow" aria-hidden="true">▸</span>{/if}
      </div>
    {/if}
  {/each}
</div>
{#if open}
  {#key open.index}
    <Self entries={itemsAt(open.index)} row={open.row} autofocus={open.viaKeyboard} {onclose} onback={closeSubmenu} />
  {/key}
{/if}

<style>
  .menu {
    position: fixed;
    z-index: 100;
    min-width: 190px;
    padding: 4px;
    visibility: hidden;
    font-size: 12px;
    color: var(--jojo-fg);
    background: var(--jojo-menu);
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    box-shadow: var(--jojo-shadow);
    outline: none;
  }
  .menu.placed {
    visibility: visible;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 5px 10px;
    border-radius: 5px;
    white-space: nowrap;
    cursor: default;
  }
  .row.highlighted {
    background: var(--jojo-sel);
  }
  .row.disabled {
    color: var(--jojo-muted);
  }
  .label {
    flex: 1;
  }
  .shortcut,
  .arrow {
    font-size: 11px;
    color: var(--jojo-muted);
  }
  .separator {
    height: 1px;
    margin: 4px 6px;
    background: var(--jojo-line);
  }
</style>
