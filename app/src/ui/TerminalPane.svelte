<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { Terminal, type ITheme } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { Unicode11Addon } from "@xterm/addon-unicode11";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import { WebglAddon } from "@xterm/addon-webgl";
  import type { SessionInfo } from "../lib/protocol/SessionInfo";
  import { app } from "../lib/state/app.svelte";
  import { createPaneStream } from "../lib/terminal/paneStream";
  import { noScreenMessage } from "../lib/terminal/pane";
  import { suppressQueryReplies } from "../lib/terminal/queries";
  import { router } from "../lib/terminal/router";
  import { clampSize } from "../lib/terminal/size";
  import { sendControl, sendInput } from "../lib/wire/bridge";

  let { session, terminalTheme }: { session: SessionInfo; terminalTheme: ITheme } = $props();
  let host: HTMLDivElement;

  onMount(() => {
    const settings = app.settings;
    const term = new Terminal({
      allowProposedApi: true,
      fontFamily: settings?.terminalFontFamily ?? "Geist Mono",
      fontSize: settings?.terminalFontSize ?? 13,
      lineHeight: settings?.terminalLineHeight ?? 1.25,
      scrollback: Math.max(settings?.scrollbackLines ?? 10_000, 10_000),
      theme: terminalTheme,
      cursorBlink: true,
    });
    const fit = new FitAddon();
    term.loadAddon(fit);
    term.loadAddon(new Unicode11Addon());
    term.unicode.activeVersion = "11";
    // The addon's default handler calls window.open, which the main webview
    // denies (see main.rs). The open_url command validates the URL (http and
    // https only) and opens it in the default browser instead.
    term.loadAddon(new WebLinksAddon((_event, uri) => void invoke("open_url", { url: uri }).catch(() => {})));
    suppressQueryReplies(term);
    term.open(host);
    let webgl: WebglAddon | undefined;
    try {
      webgl = new WebglAddon();
      term.loadAddon(webgl);
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

    const id = session.id;
    let size = clampSize(term.cols, term.rows);
    const stream = createPaneStream({
      write: (bytes, done) => term.write(bytes, done),
      sendAttach: () => sendControl({ type: "Attach", session_id: id, cols: size.cols, rows: size.rows }),
    });
    // A session with no screen is never attached, so it is not registered
    // with the router: reattachAll() after a reconnect must not send an
    // Attach that jojod can only refuse.
    const unregister = session.attachable ? router.register(id, stream) : () => {};
    if (session.attachable) stream.start();
    else term.write(`\x1b[2m${noScreenMessage(session)}\x1b[0m\r\n`);

    // A session with no screen has nothing on the daemon side to receive
    // input, so keystrokes are not sent. App.svelte keys this pane on
    // paneKey(), which includes `attachable`, so a session that loses its
    // screen while mounted is remounted and reaches this check.
    const encoder = new TextEncoder();
    const onData = term.onData((data) => {
      if (!session.attachable) return;
      void sendInput(id, encoder.encode(data)).catch(() => {});
    });
    const onBinary = term.onBinary((data) => {
      if (!session.attachable) return;
      void sendInput(id, Uint8Array.from(data, (c) => c.charCodeAt(0) & 0xff)).catch(() => {});
    });

    let timer: ReturnType<typeof setTimeout> | undefined;
    const observer = new ResizeObserver(() => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        // Always re-fit so a session with no screen still wraps its static
        // message to the host's width; only an attachable session has
        // anything on the daemon side to tell about the new size.
        fit.fit();
        const next = clampSize(term.cols, term.rows);
        if (next.cols !== size.cols || next.rows !== size.rows) {
          size = next;
          if (session.attachable) {
            void sendControl({ type: "Resize", session_id: id, cols: size.cols, rows: size.rows }).catch(() => {});
          }
        }
      }, 50);
    });
    observer.observe(host);
    term.focus();

    return () => {
      observer.disconnect();
      clearTimeout(timer);
      unregister();
      onData.dispose();
      onBinary.dispose();
      if (session.attachable) void sendControl({ type: "Detach", session_id: id }).catch(() => {});
      webgl?.dispose();
      term.dispose();
    };
  });
</script>

<div class="frame">
  <div class="host" bind:this={host}></div>
</div>

<style>
  .frame {
    position: absolute;
    inset: 0;
    padding: 10px 12px;
    background: var(--jojo-pane);
  }
  .host {
    width: 100%;
    height: 100%;
  }
</style>
