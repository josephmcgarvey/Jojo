<script lang="ts">
  import { onMount } from "svelte";
  import { Terminal, type ITheme } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { Unicode11Addon } from "@xterm/addon-unicode11";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import { WebglAddon } from "@xterm/addon-webgl";
  import type { SessionInfo } from "../lib/protocol/SessionInfo";
  import { app, machineOfSession } from "../lib/state/app.svelte";
  import { noScreenMessage } from "../lib/terminal/pane";
  import { contextMenuTarget } from "../lib/input/mouse";
  import { resolveShortcut } from "../lib/input/shortcuts";
  import { copyFromPane } from "../lib/state/clipboard";
  import { openLink } from "../lib/terminal/links";
  import { createPaneStream } from "../lib/terminal/paneStream";
  import { suppressQueryReplies } from "../lib/terminal/queries";
  import { registerTerminal } from "../lib/terminal/registry";
  import { router } from "../lib/terminal/router";
  import { clampSize } from "../lib/terminal/size";
  import { sendControl, sendInput } from "../lib/wire/bridge";

  let {
    session,
    terminalTheme,
    focused,
    onmenu,
  }: { session: SessionInfo; terminalTheme: ITheme; focused: boolean; onmenu: (x: number, y: number) => void } = $props();
  let host: HTMLDivElement;
  /** Set in onMount and cleared on unmount; read by the focus effect below. */
  let term: Terminal | undefined;

  onMount(() => {
    const settings = app.settings;
    const id = session.id;
    // The session's machine; every request and keystroke for it goes there.
    const machine = machineOfSession(id);
    const created = new Terminal({
      allowProposedApi: true,
      fontFamily: settings?.terminalFontFamily ?? "Geist Mono",
      fontSize: settings?.terminalFontSize ?? 13,
      lineHeight: settings?.terminalLineHeight ?? 1.25,
      scrollback: Math.max(settings?.scrollbackLines ?? 10_000, 10_000),
      theme: terminalTheme,
      cursorBlink: true,
      // OSC 8 hyperlinks. xterm.js's default handler calls confirm() and then
      // window.open(), which the main window denies, so nothing opened.
      // allowNonHttpProtocols stays false, so links with other schemes are
      // ignored by xterm.js.
      linkHandler: { activate: (_event, uri) => void openLink(id, uri, true) },
    });
    term = created;
    const fit = new FitAddon();
    created.loadAddon(fit);
    created.loadAddon(new Unicode11Addon());
    created.unicode.activeVersion = "11";
    // Plain URLs found in the output. Their text is the URL itself, so they
    // open without a confirmation; open_url validates them.
    created.loadAddon(new WebLinksAddon((_event, uri) => void openLink(id, uri, false)));
    suppressQueryReplies(created);
    created.open(host);
    // Spec 9.4 mouse reporting rule. This listener runs in the capture phase
    // on the host, before xterm.js's handlers on its child elements. The
    // browser's own context menu is always suppressed. When the program has
    // mouse reporting on and Shift is not held, xterm.js has already
    // reported the right-button press to the program and nothing else
    // happens here.
    const onContextMenu = (event: MouseEvent) => {
      event.preventDefault();
      if (contextMenuTarget(event.shiftKey, created.modes.mouseTrackingMode) === "jojo") onmenu(event.clientX, event.clientY);
    };
    host.addEventListener("contextmenu", onContextMenu, true);
    // App.svelte handles application shortcuts on keydown before xterm.js
    // sees them; returning false here makes xterm.js also ignore the
    // keypress and keyup events of those keys.
    created.attachCustomKeyEventHandler(
      (event) => resolveShortcut(event, { copyOnSelect: app.settings?.copyOnSelect ?? true, hasSelection: created.hasSelection() }) === null,
    );
    // Spec 9.5 rules 1-2: releasing the left button after a drag, double-click
    // or triple-click selection copies it. xterm.js tracks a drag with
    // document-level listeners, so the button can be released outside this
    // host (in the pane padding, or past the pane while autoscrolling); the
    // mouseup is therefore listened for on window, armed by a left-button
    // press inside the host. The check waits one task so that xterm.js's own
    // mouseup handling has finished the selection.
    const onWindowMouseUp = (event: MouseEvent) => {
      if (event.button !== 0) return;
      window.removeEventListener("mouseup", onWindowMouseUp, true);
      if (!(app.settings?.copyOnSelect ?? true)) return;
      setTimeout(() => {
        if (created.hasSelection()) void copyFromPane(id);
      }, 0);
    };
    const onMouseDown = (event: MouseEvent) => {
      if (event.button === 0) window.addEventListener("mouseup", onWindowMouseUp, true);
    };
    host.addEventListener("mousedown", onMouseDown);
    let webgl: WebglAddon | undefined;
    try {
      webgl = new WebglAddon();
      created.loadAddon(webgl);
      // xterm.js falls back to its DOM renderer on context loss; the addon
      // itself does not dispose on loss, so do it here to release the GL
      // resources instead of leaving a dead context attached.
      webgl.onContextLoss(() => {
        webgl?.dispose();
        webgl = undefined;
      });
    } catch {
      // WebGL unavailable: xterm.js keeps its DOM renderer.
      webgl = undefined;
    }
    fit.fit();

    let size = clampSize(created.cols, created.rows);
    const stream = createPaneStream({
      write: (bytes, done) => created.write(bytes, done),
      sendAttach: () => sendControl({ type: "Attach", session_id: id, cols: size.cols, rows: size.rows }, machine),
    });
    // A session with no screen is never attached, so it is not registered
    // with the router: reattachAll() after a reconnect must not send an
    // Attach that jojod can only refuse.
    const unregister = session.attachable ? router.register(id, stream) : () => {};
    if (session.attachable) stream.start();
    else created.write(`\x1b[2m${noScreenMessage(session)}\x1b[0m\r\n`);

    // Fits the terminal to the host and tells the daemon about a new size.
    // Always re-fits so a session with no screen still wraps its static
    // message to the host's width; only an attachable session has anything
    // on the daemon side to tell about the new size.
    const refit = () => {
      fit.fit();
      const next = clampSize(created.cols, created.rows);
      if (next.cols !== size.cols || next.rows !== size.rows) {
        size = next;
        if (session.attachable) {
          void sendControl({ type: "Resize", session_id: id, cols: size.cols, rows: size.rows }, machine).catch(() => {});
        }
      }
    };
    const unregisterControls = registerTerminal(id, {
      focus: () => created.focus(),
      hasSelection: () => created.hasSelection(),
      getSelection: () => created.getSelection(),
      paste: (text) => created.paste(text),
      mouseTrackingMode: () => created.modes.mouseTrackingMode,
      applyFont: (family, fontSize, lineHeight) => {
        created.options.fontFamily = family;
        created.options.fontSize = fontSize;
        created.options.lineHeight = lineHeight;
        // A new cell size changes how many columns and rows fit; the resize
        // observer does not fire because the host's size is unchanged.
        refit();
      },
      rehydrate: () => {
        if (session.attachable) stream.reattach(true);
      },
    });

    // A session with no screen has nothing on the daemon side to receive
    // input, so keystrokes are not sent. PaneGrid keys this pane on
    // paneKey(), which includes `attachable`, so a session that loses its
    // screen while mounted is remounted and reaches this check.
    const encoder = new TextEncoder();
    const onData = created.onData((data) => {
      if (!session.attachable) return;
      void sendInput(id, encoder.encode(data), machine).catch(() => {});
    });
    const onBinary = created.onBinary((data) => {
      if (!session.attachable) return;
      void sendInput(id, Uint8Array.from(data, (c) => c.charCodeAt(0) & 0xff), machine).catch(() => {});
    });

    let timer: ReturnType<typeof setTimeout> | undefined;
    const observer = new ResizeObserver(() => {
      clearTimeout(timer);
      timer = setTimeout(refit, 50);
    });
    observer.observe(host);

    return () => {
      host.removeEventListener("contextmenu", onContextMenu, true);
      host.removeEventListener("mousedown", onMouseDown);
      window.removeEventListener("mouseup", onWindowMouseUp, true);
      stream.stop();
      observer.disconnect();
      clearTimeout(timer);
      unregister();
      unregisterControls();
      onData.dispose();
      onBinary.dispose();
      if (session.attachable) void sendControl({ type: "Detach", session_id: id }, machine).catch(() => {});
      webgl?.dispose();
      term = undefined;
      created.dispose();
    };
  });

  // Moves keyboard focus into this terminal whenever its pane becomes the
  // focused pane: a click on the pane, Alt+Arrow, a sidebar click, or a tab
  // switch. Declared after onMount, so on first mount it runs after `term`
  // is set.
  $effect(() => {
    if (focused) term?.focus();
  });
</script>

<div class="host" bind:this={host}></div>

<style>
  .host {
    width: 100%;
    height: 100%;
  }
</style>
