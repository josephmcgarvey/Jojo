# Jojo Design Pass: Neon Glass, Bold

The user reviewed three full-window comps of the finished app (`.superpowers/brainstorm/1391-1794187176/content/design-directions.html`) on 2026-11-08 and chose **C · Neon Glass, bold**, with two changes: the **+** icons become a filled circle with the plus knocked out (the Heroicons solid `plus-circle` shape) and larger, and the wordmark reads **Jojo**. This document is the visual specification the implementation follows. It changes styling only; no behavior changes.

## 1. Surfaces

1. **Window background:** three radial glows over `#060816`: violet `rgba(155,140,255,.55)` at the top right (700 × 420 px ellipse at 90% −20%), teal `rgba(62,230,176,.32)` at the bottom left (520 × 380 px at 0% 120%), and a faint cyan `rgba(127,212,255,.12)` in the middle (400 × 300 px at 50% 60%).
2. **Sidebar:** a floating frosted panel, 8 px from the window's top, bottom and left edges, radius 10 px, background `rgba(20,26,54,.42)` with `backdrop-filter: blur(16px)`, and a 1 px border `rgba(170,180,255,.16)`. Width 260 px.
3. **Panes:** separate frosted cards, radius 12 px, background `rgba(8,11,24,.72)` with `backdrop-filter: blur(10px)`, 1 px border `rgba(170,180,255,.16)`, 10 px apart. The focused pane (spec 9.3 rule 4) has a border `rgba(155,140,255,.7)`, an outer 1 px ring `rgba(155,140,255,.35)` and a 28 px glow `rgba(155,140,255,.25)`. The terminal inside stays opaque (its theme background), because a translucent WebGL terminal costs rendering time and lowers text contrast.
4. **Lines:** every divider and border uses `rgba(170,180,255,.16)`.

## 2. Components

1. **Wordmark:** "Jojo", 17 px, weight 600, gradient `#d6cfff → #7fd4ff (60%) → #3ee6b0`.
2. **+ icons:** an inline SVG of the Heroicons solid `plus-circle` (24 × 24 viewBox, MIT license, no dependency), drawn at 18 px in `currentColor`: the machine header's add-project button, each project's new-terminal button and the tab bar's new-tab button. Muted by default, the foreground color on hover, the accent color while pressed.
3. **Session rows:** the selected row has a gradient from `rgba(155,140,255,.28)` to `rgba(155,140,255,.06)`, radius 8 px.
4. **Tabs:** no raised tab shape. Each tab is text with a 2 px bottom border, transparent by default; the active tab's border is the accent color and its text has a 12 px glow `rgba(155,140,255,.6)`. The tab row has no background of its own (the window glow shows through).
5. **Pane header strip:** inside the card at its top, 10.5 px muted text, the badge, the label, and the project name on the right.
6. **Status bar:** no background; the machine label and the project path are pills (radius 999 px, background `rgba(20,26,54,.6)`, 1 px line border); the switcher hint on the right shows `Ctrl+Shift+P` as a key cap.
7. **Badges, dialogs, menus:** unchanged, apart from using the new line color.

## 3. Implementation

All values become `--jojo-*` tokens in `app/src/themes/neon-glass.json` (`bg-glow`, `line`, `glass-side`, `glass-pane`, `focus-ring`, `brand-gradient`, `radius-lg`, `pill`), so the color lint keeps passing and a later theme can replace them. The + icon is one Svelte component, `app/src/ui/PlusCircle.svelte`.
