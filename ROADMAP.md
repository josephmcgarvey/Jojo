# Jojo roadmap

This file lists work that has been agreed on but deliberately postponed. Each
entry states what the feature is, what building it involves, and what it
depends on. Completed work is recorded in the git history and the plans under
`docs/superpowers/plans/`, not here.

## 1. Four new themes

**Status:** design approved (2026-11-10), not started. The studies are in the
artifact "Jojo Theme Studies" (https://claude.ai/artifact/FypJBXuKNUwxHYCrkgfgy1).

Jojo has two themes today, Neon Glass (dark) and Daylight (light). Both share
one structure: frosted glass panels with `backdrop-filter: blur`, a floating
sidebar, glows, and rounded corners. The Settings ▸ Base color control only
changes their hues (`app/src/lib/palette.ts`). The four new themes change the
structure as well as the colors.

| Theme | Mode | Structure | Colors and type |
|---|---|---|---|
| Graphite | Dark | Flat and matte: no glass, no glow. 4 px corners. The sidebar is a flat column against the window edge, not a floating panel. | Black, white and gray interface (`#111113` background, `#ececef` text and accent); color appears only for session states. Inter for the interface. |
| Phosphor | Dark | Square corners (0 px). Soft glow on text. | Green-screen palette (`#050806` background, `#c6f7d3` text, `#7dffa8` accent). State colors tuned for it: working `#6fe3ff`, needs you `#ffbf3c`, done `#a6ff4d` (chartreuse, so it does not blend into the green text), error `#ff5f56`. IBM Plex Mono for the interface and the terminal. |
| Porcelain | Light | Opaque white panels on a gray background (`#e9ecf2`), soft shadows instead of glows, 12–14 px corners. | Cobalt accent `#2f5bea`; dark text `#141824`. Light and solid, where Daylight is light and glassy. |
| Contrast | Dark | Pure black and white, 2 px borders, bold headings. The sidebar is a flat column. | Yellow focus and accent `#ffd400`, borders `#8a8a8a`, saturated state colors. Intended for maximum legibility. |

**What building them involves:**

1. **Theme-controlled structure.** The glass blur, the floating sidebar
   margins, the border width and the corner radii are written into component
   styles today. Each must become a theme token (for example `glass-blur`,
   `side-gap`, `border-width`, and the existing `radius` tokens), so Graphite
   and Contrast can be flat and edge-to-edge.
2. **Per-theme state colors.** The state tokens (`working`, `needs-you`,
   `done`, `error`) already live in each theme file; Phosphor and Contrast set
   their own values.
3. **Per-theme interface font.** A theme sets the default interface font
   (Phosphor uses IBM Plex Mono). The Settings ▸ Interface font choice still
   overrides it, as it does now. IBM Plex Mono is already bundled.
4. **Base color.** Settings ▸ Base color keeps working on every theme; the
   palette code recolors around each theme's `accent`.
5. **Tests.** The existing contrast tests in `app/src/lib/theme.test.ts` run
   over every theme in `THEMES`, so each new theme must meet the same floors
   (text 7:1, secondary text 4.5:1, primary button 4.5:1) under its own colors
   and under every base color preset.

## 2. Jojo logo at the top of the sidebar

**Status:** waiting for the SVG from the user.

The "Jojo" wordmark was removed from the top of the sidebar. A logo replaces
it once the user provides the artwork. Requirements for the file: a `viewBox`
with no fixed `width` or `height` (the app sizes it to about 24–28 px tall);
either `currentColor`, so it follows the theme, or fixed colors that read on
both dark and light themes; text converted to outlines. On Windows it sits at
the top left of the sidebar above the Needs you queue; on macOS it sits to the
right of the window's traffic-light buttons.

## 3. Last line of output on session cards

**Status:** deferred from the agents-first layout (commit `a1cb61f`).

The approved mock-up showed each session card with its session's last line of
terminal output. The app holds a terminal screen only for panes in the open
tab, so the cards show the session's state detail (for example the permission
prompt) or its project name instead. Showing the last line for every session
needs jojod to send it, for example as a field on `SessionInfo` updated from
the session's grid, throttled so a busy session does not flood the
connection.

## 4. Terminal color replies that follow the theme

**Status:** known limitation (commit `c8ccca6`).

jojod answers programs' color queries (OSC 4, 10, 11 and 12), so replies
arrive in order and do not leak into the shell. It answers with the Neon Glass
colors because it does not know which theme or base color the window shows.
Under Daylight, a program that adapts to the background (`gh`, editors) is
told the background is dark. Fixing it needs the window to send its terminal
colors to the daemon, a new control message.

## 5. macOS version

**Status:** built once, then paused at the user's request (2026-11-10).

The app and jojod run on macOS (commit `eb69807`), and
`.github/workflows/macos.yml` builds unsigned `.dmg` installers for Apple
Silicon and Intel. The workflow runs only when started by hand
(`gh workflow run macos.yml -R josephmcgarvey/Jojo`). No Mac build is made
until the user asks for one. Never verified on a real Mac: the traffic-light
position against the sidebar, starting the daemon through `zsh -l -i`, and
notifications for an unsigned app.
