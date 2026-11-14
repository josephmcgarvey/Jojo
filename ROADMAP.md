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

## 3. macOS version checks on a real Mac

**Status:** Mac installers ship with every release since 0.2.0; three
behaviors have not been confirmed on a real Mac.

The app and jojod run on macOS (commit `eb69807`), and
`.github/workflows/macos.yml` builds unsigned `.dmg` installers for Apple
Silicon and Intel. The workflow runs only when started by hand
(`gh workflow run macos.yml -R josephmcgarvey/Jojo --ref <tag>`), and Mac
builds are made only when the user asks for one. These behaviors are
unconfirmed:

1. The position of the traffic-light buttons against the sidebar's JOJO
   logo (`trafficLightPosition` in `app/src-tauri/tauri.macos.conf.json`).
2. Starting the daemon through the login shell (`zsh -l -i`), which is how
   jojod finds Claude Code installed through Homebrew, nvm or an alias.
3. Notifications from an unsigned app.

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

## 6. Choosing an agent per session

**Status:** requested by the user on 2026-11-12; postponed. Compact
conversation was built for Claude Code only (the session menu and the
context meter's right-click menu type `/compact`).

A session is either `Claude` or `Shell` today (`SessionKind` in
`crates/jojo-proto/src/msg.rs`). Supporting other coding agents means a
session records which agent it runs, and each agent has a profile that
says:

1. **Launch command:** for example `claude`, `codex` or `gemini`, with the
   arguments for resuming a conversation where the agent has them.
2. **State reporting:** how the agent tells jojod it is working, waiting
   for the user or done. Claude Code reports through hooks registered in
   `~/.claude/settings.json` (`jojod install-hooks`). Each other agent's
   mechanism, if it has one, has to be checked agent by agent; an agent
   without one shows only Running and Exited, as a shell does.
3. **Context source:** where the context meter's token count and window
   size come from. For Claude Code, jojod reads the transcript named by the
   hooks (`crates/jojo-agent/src/context.rs`). An agent without a source
   has no meter.
4. **Compact command:** the text Compact conversation types, if the agent
   has one. Claude Code and OpenAI Codex CLI use `/compact`; Google Gemini
   CLI uses `/compress` (both unverified against current versions). An
   agent without one has no Compact item.

The change replaces `SessionKind::Claude` with an agent id in the protocol
(a protocol version change, with old daemons answered as for other
version mismatches), adds the agent choice to the New session menu, and
moves Compact conversation's command and enabled states into the profile.

## 7. Natural-language search of session output

**Status:** postponed by the user, who chose plain-text search first
(built in the switcher's In output section).

Plain-text search finds lines containing the typed words in every
session's grid and scrollback (`jojo_grid::Grid::search`, at most 20 matches
per session and 200 in total). Natural-language search would answer
questions such as "which session was working on the migration?". It needs
the session text sent to a model, which raises what is sent and where: the
output can contain secrets and client data, so the design has to say which
model runs it, whether it runs locally, and how a session is excluded.

## 8. Snippet placeholders and shared snippets

**Status:** postponed when the snippets library was built (commit
`4c47cfb`).

1. Placeholders: a snippet such as `git checkout {{branch}}` asks for each
   `{{name}}` value in a small form before inserting, and remembers the last
   value used.
2. Shared snippets: snippets kept in a project's repository, for example
   `.jojo/snippets.json`, listed for terminals in that project and shared
   with other developers through git. The window would read the file over
   the project's location (jojod's file access on a remote server), and
   edits would be written back to it.

Today snippets live only in `snippets.json` in the app's settings folder,
one library per computer.

## 9. Showing why menu items are disabled

**Status:** raised by the user on 2026-11-14; postponed.

Pane, tab and session menu items that cannot apply are grayed out, and the
reason (for example "Only pane in this tab" or "The current tab belongs to
another project") appears only as a tooltip on hover
(`disabledReason` in `app/src/lib/menu/menus.ts`). Two options:

1. Show the reason as muted text on the item's row.
2. Hide items that can never apply in the current tab (Zoom, Swap with,
   Layout and Equalize sizes in a tab with one pane), and keep graying out
   items that can apply after a change, such as Open in split.
