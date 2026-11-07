<script lang="ts">
  import { onMount } from "svelte";
  import { Terminal, type ITheme } from "@xterm/xterm";
  import { FitAddon } from "@xterm/addon-fit";
  import { Unicode11Addon } from "@xterm/addon-unicode11";
  import { WebLinksAddon } from "@xterm/addon-web-links";
  import { WebglAddon } from "@xterm/addon-webgl";
  import type { SessionInfo } from "../lib/protocol/SessionInfo";
  import { app } from "../lib/state/app.svelte";
  import { AttachGate } from "../lib/terminal/attachGate";
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
    term.loadAddon(new WebLinksAddon());
    suppressQueryReplies(term);
    term.open(host);
    try {
      term.loadAddon(new WebglAddon());
    } catch {
      // WebGL unavailable: xterm.js keeps its DOM renderer.
    }
    fit.fit();

    const id = session.id;
    const gate = new AttachGate();
    let size = clampSize(term.cols, term.rows);
    const attach = () => {
      if (!session.attachable) return;
      gate.requestAttach();
      void sendControl({ type: "Attach", session_id: id, cols: size.cols, rows: size.rows });
    };
    if (session.attachable) attach();
    else term.write("\x1b[2mThis session has no screen: it ran in a previous jojod. Close it and start a new one.\x1b[0m\r\n");

    const unregister = router.register(id, {
      deliver(kind, bytes) {
        const accepted = gate.accept(kind, bytes);
        if (accepted) term.write(accepted);
      },
      reattach: attach,
    });

    const encoder = new TextEncoder();
    const onData = term.onData((data) => void sendInput(id, encoder.encode(data)));
    const onBinary = term.onBinary((data) => void sendInput(id, Uint8Array.from(data, (c) => c.charCodeAt(0) & 0xff)));

    let timer: ReturnType<typeof setTimeout> | undefined;
    const observer = new ResizeObserver(() => {
      clearTimeout(timer);
      timer = setTimeout(() => {
        fit.fit();
        const next = clampSize(term.cols, term.rows);
        if (next.cols !== size.cols || next.rows !== size.rows) {
          size = next;
          void sendControl({ type: "Resize", session_id: id, cols: size.cols, rows: size.rows });
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
      if (session.attachable) void sendControl({ type: "Detach", session_id: id });
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
