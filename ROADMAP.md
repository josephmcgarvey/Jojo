# Jojo roadmap

This file lists work that has been agreed on but deliberately postponed. Each
entry states what the feature is, what building it involves, and what it
depends on. Completed work is recorded in the git history and the plans under
`docs/superpowers/plans/`, not here.

## 1. Last line of output on session cards

**Status:** deferred from the agents-first layout (commit `a1cb61f`).

The approved mock-up showed each session card with its session's last line of
terminal output. The app holds a terminal screen only for panes in the open
tab, so the cards show the session's state detail (for example the permission
prompt) or its project name instead. Showing the last line for every session
needs jojod to send it, for example as a field on `SessionInfo` updated from
the session's grid, throttled so a busy session does not flood the
connection.

## 2. Terminal color replies that follow the theme

**Status:** known limitation (commit `c8ccca6`).

jojod answers programs' color queries (OSC 4, 10, 11 and 12), so replies
arrive in order and do not leak into the shell. It answers with the Neon Glass
colors because it does not know which theme or base color the window shows.
Under Daylight, a program that adapts to the background (`gh`, editors) is
told the background is dark. Fixing it needs the window to send its terminal
colors to the daemon, a new control message.

## 3. macOS version

**Status:** built once, then paused at the user's request (2026-11-10).

The app and jojod run on macOS (commit `eb69807`), and
`.github/workflows/macos.yml` builds unsigned `.dmg` installers for Apple
Silicon and Intel. The workflow runs only when started by hand
(`gh workflow run macos.yml -R josephmcgarvey/Jojo`). No Mac build is made
until the user asks for one. Never verified on a real Mac: the traffic-light
position against the sidebar, starting the daemon through `zsh -l -i`, and
notifications for an unsigned app.

## 4. Video demo with voiceover

**Status:** postponed by the user (2026-11-10); approach not chosen.

Two ways to make it, which differ in how the screen is recorded:

1. **Record the real app, then add a voiceover.** Claude writes a script
   (scenes, the clicks in each, narration text and rough timings). The user
   records the screen while following it, with the Windows 11 Snipping Tool
   (video mode) or OBS, using real Claude sessions. The voiceover is either
   the user's own recording or generated from the script with ElevenLabs
   (needs an API key). Claude assembles the result with ffmpeg: trims, title
   and end cards, the voice track, music ducked under the voice, and
   captions. No new code. Recommended for a first video.
2. **Scripted recording with the `demo-video-kit` plugin** (installed,
   version 0.15.0). It drives the app with Playwright and records it with an
   animated cursor, click ripples, captions, zoom and voiceover timed to each
   step, and can re-record after changes with one command. It records web
   apps in a browser, so Jojo would need a browser "demo mode" of its
   interface with made-up sessions and scripted terminal output, because the
   real interface depends on the Rust backend and live terminals. On Windows
   its voices come from ElevenLabs or a recorded file (its built-in `say`
   voice is macOS only), and it needs the Playwright MCP server, which failed
   to connect on 2026-11-10.

Both need ffmpeg (`winget install Gyan.FFmpeg`).

**Open questions for the user:**

1. Which approach.
2. The user's own voice or an AI voice; for an AI voice, whether there is an
   ElevenLabs account.
3. The audience and length, for example a 60–90 second overview for a
   website or a 3–5 minute walkthrough for people trying Jojo.

## 5. Clear error for servers that run each SSH connection in a container

**Status:** found on 2026-11-12 on client-shared-host; the server was fixed by
changing the system user's shell, and the app change is postponed.

RunCloud's jailed shell (`/bin/rc-shell`) runs every SSH connection in its
own temporary Docker container: a separate process namespace (process 1 is
`bash`), the hostname `localhost` and a `/.dockerenv` file. The home folder
is shared between connections; processes are not. Jojo starts the remote
daemon (`jojod serve --daemonize`) over one connection and attaches over
another (`jojod attach --stdio`). The daemon dies with the first connection's
container, the attach finds `~/.jojo/jojod.sock` with no listener, and the
window retries about every 33 seconds while showing "Reconnecting"
indefinitely, with no explanation. The server's log showed a new
`jojod starting` line every 33 seconds and no client connections.

The fix is to detect this case and report it instead of retrying:

1. During **Add remote location** and on each reconnect, check for
   `/.dockerenv` or a process 1 that is not an init system, and check whether
   a daemon started in one connection is still running in the next.
2. When it is not, stop retrying and show a message such as "This server
   runs each SSH connection in a temporary container, so the Jojo daemon and
   its sessions cannot keep running. Change the user's shell to /bin/bash
   (RunCloud: System Users)."

Running the daemon inside the attach connection would make such servers
usable, but sessions would end whenever the window disconnects, so it is not
part of this fix.

## 6. Folding locations and projects in the sidebar

**Status:** mock-up made on 2026-11-12
(https://claude.ai/artifact/5JdLfPsp4LdEntkeGPoMgk); postponed by the user
before building.

Each location (This PC and each remote location) and each project in the
sidebar can be folded to hide the rows beneath it. The design is meant to be
barely visible until used:

1. A 9-pixel chevron appears in the row's left margin while the pointer is
   over a location or project row. It sits in the row's existing padding, so
   no text moves.
2. Clicking the location name or the project name folds or unfolds it.
   Clicking a project name also still selects the project.
3. A folded row keeps its chevron at low opacity, pointing right, which is
   what separates a folded group from an empty one.
4. In place of the hidden rows, a folded row shows one 5-pixel dot per
   session that is not idle, in its state color. A session that needs the
   user keeps its amber pulse, so folding never hides a waiting session. A
   folded location also shows its project count ("2 projects"), and its
   dots cover all of its projects' sessions.
5. The fold animates over 160 milliseconds, with no animation under
   `prefers-reduced-motion`.
6. With a row focused, Left folds and Right unfolds it. Alt+click folds or
   unfolds every project under the same location.
7. The Needs you queue still lists waiting sessions from folded groups.
   Choosing one unfolds its location and project and focuses its pane.
8. Folded groups are saved in `layout.json` (a list of folded location and
   project ids, with a serde default so older files still load), so they
   stay folded after a restart.

**Open question for the user:** whether a click on a project name both
selects and folds it, as in the mock-up, or only the chevron folds it. The
chevron alone keeps selecting and folding separate, but its click target is
much smaller.
