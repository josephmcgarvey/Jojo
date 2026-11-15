<script lang="ts">
  // Spec 8.2: the Add machine dialog, shown as "Add remote location". The user picks a host alias from
  // ~/.ssh/config (or types user@host) and a label; Install runs the steps
  // in the backend (connectivity, platform, install or upgrade, start,
  // logout survival, hooks) and shows each one as it finishes.
  import { onMount } from "svelte";
  import { Channel, invoke } from "@tauri-apps/api/core";
  import type { InstallStep } from "../lib/backend/InstallStep";
  import type { Machine } from "../lib/backend/Machine";
  import { errorText } from "../lib/errors";
  import { app, machineAdded } from "../lib/state/app.svelte";
  import { isMac } from "../lib/platform";

  let { onclose }: { onclose: () => void } = $props();

  const STEP_NAMES: Record<string, string> = {
    connect: "Connect",
    key: "SSH key",
    platform: "Platform",
    install: "Install jojod",
    start: "Start jojod",
    linger: "Logout survival",
    hooks: "Claude Code hooks",
  };

  let hosts = $state<string[]>([]);
  let host = $state("");
  /** The Host select's value for a host typed into the text field. */
  const OTHER_HOST = "__other__";
  let otherHost = $state(false);
  const hostChoice = $derived(otherHost ? OTHER_HOST : host);
  /** Hosts already in the sidebar, marked in the list (adding one again upgrades and reconnects it). */
  const added = $derived(new Set(app.machines.map((m) => m.ssh_host).filter((h): h is string => !!h)));
  function chooseHost(value: string): void {
    otherHost = value === OTHER_HOST;
    host = otherHost ? "" : value;
  }
  /**
   * How Jojo signs in: "key", a host from ~/.ssh/config whose key is
   * loaded in the SSH agent; or "password", a username and password, with
   * which Jojo installs its own key (setupKey) or, when the server refuses
   * a key or setupKey is off, keeps the password in the system credential
   * store.
   */
  let signInWith = $state<"key" | "password">("key");
  let address = $state("");
  let port = $state(22);
  let user = $state("");
  let password = $state("");
  let setupKey = $state(true);
  const ready = $derived(signInWith === "key" ? host.trim() !== "" : address.trim() !== "" && user.trim() !== "" && password !== "");
  let label = $state("");
  let running = $state(false);
  let done = $state(false);
  let error = $state<string | null>(null);
  /** Set when installing would end a running daemon of another protocol (spec 8.4). */
  let restartNeeded = $state(false);
  let steps = $state<InstallStep[]>([]);

  onMount(() => {
    void invoke<string[]>("ssh_hosts")
      .then((list) => {
        hosts = list;
        // With no ~/.ssh/config hosts, the only choice is typing one.
        if (list.length === 0) otherHost = true;
      })
      .catch(() => (otherHost = true));
  });

  /** The backend's prefix for "installing would end the running daemon" (install.rs RESTART_NEEDED). */
  const RESTART_NEEDED = "RESTART_NEEDED: ";

  async function install(allowRestart = false): Promise<void> {
    running = true;
    done = false;
    error = null;
    restartNeeded = false;
    steps = [];
    const progress = new Channel<InstallStep>();
    progress.onmessage = (step) => {
      // One row per step: a later report of the same step replaces the earlier one.
      const index = steps.findIndex((s) => s.step === step.step);
      if (index >= 0) steps[index] = step;
      else steps.push(step);
    };
    try {
      const signIn = signInWith === "password" ? { address, port: Number(port) || 22, user, password, setupKey } : null;
      const machine = await invoke<Machine>("add_machine", { host: signInWith === "key" ? host : "", label, allowRestart, signIn, progress });
      password = "";
      machineAdded(machine);
      done = true;
    } catch (err) {
      const text = errorText(err);
      restartNeeded = text.startsWith(RESTART_NEEDED);
      error = restartNeeded ? text.slice(RESTART_NEEDED.length) : text;
    } finally {
      running = false;
    }
  }

  function onKeydown(event: KeyboardEvent): void {
    if (event.key === "Escape" && !running) onclose();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="scrim jojo-scrim" onkeydown={onKeydown}>
  <div class="dialog jojo-modal" role="dialog" aria-modal="true" aria-labelledby="add-machine-title">
    <h2 id="add-machine-title">Add remote location</h2>
    <p class="hint">A Linux server (x86_64 or aarch64) reached over SSH. Jojo installs <code>~/.local/bin/jojod</code> there.</p>
    <div class="signin" role="radiogroup" aria-label="Sign in with">
      <span>Sign in with</span>
      <label class="choice"><input type="radio" bind:group={signInWith} value="key" disabled={running || done} /> SSH key</label>
      <label class="choice"><input type="radio" bind:group={signInWith} value="password" disabled={running || done} /> Username and password</label>
    </div>
    {#if signInWith === "key"}
      <p class="hint">A host from <code>~/.ssh/config</code> whose key is loaded in the SSH agent, with its host key accepted once.</p>
      <label>
        <span>Host</span>
        <select value={hostChoice} onchange={(e) => chooseHost(e.currentTarget.value)} disabled={running || done}>
          {#if !otherHost && !host}<option value="" disabled>Choose a host from ~/.ssh/config</option>{/if}
          {#each hosts as alias (alias)}
            <option value={alias}>{alias}{added.has(alias) ? " (added: upgrades and reconnects)" : ""}</option>
          {/each}
          <option value={OTHER_HOST}>Other host (user@host)…</option>
        </select>
      </label>
      {#if otherHost}
        <label>
          <span>Address</span>
          <!-- svelte-ignore a11y_autofocus -->
          <input bind:value={host} placeholder="user@host or host" autofocus disabled={running || done} spellcheck="false" />
        </label>
      {/if}
    {:else}
      <label>
        <span>Host</span>
        <div class="pair">
          <input bind:value={address} placeholder="server.example.com or 192.0.2.5" disabled={running || done} spellcheck="false" aria-label="Host name or IP address" />
          <input class="port" type="number" min="1" max="65535" bind:value={port} disabled={running || done} aria-label="SSH port" />
        </div>
      </label>
      <label>
        <span>Username</span>
        <input bind:value={user} autocomplete="off" disabled={running || done} spellcheck="false" />
      </label>
      <label>
        <span>Password</span>
        <input type="password" bind:value={password} autocomplete="off" disabled={running || done} />
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={setupKey} disabled={running || done} />
        <span>Set up an SSH key so the password is not needed again (recommended). If the server does not accept the key, the password is saved in {isMac ? "the Keychain" : "Windows Credential Manager"}.</span>
      </label>
    {/if}
    <label>
      <span>Label</span>
      <input bind:value={label} placeholder={(signInWith === "key" ? host : address) || "shown in the sidebar"} disabled={running || done} />
    </label>

    {#if steps.length > 0}
      <ol class="steps">
        {#each steps as s (s.step)}
          <li class={s.status}>
            <span class="mark">{s.status === "ok" ? "✓" : s.status === "failed" ? "✕" : s.status === "warning" ? "!" : "…"}</span>
            <span class="name">{STEP_NAMES[s.step] ?? s.step}</span>
            <span class="message">{s.message.replace(RESTART_NEEDED, "")}</span>
          </li>
        {/each}
      </ol>
    {/if}
    {#if error}<p class="error">{error}</p>{/if}

    <div class="actions">
      {#if done}
        <button class="primary" onclick={onclose}>Done</button>
      {:else}
        <button onclick={onclose} disabled={running}>Cancel</button>
        {#if restartNeeded}
          <button class="primary" onclick={() => install(true)} disabled={running}>Install and end sessions</button>
        {:else}
          <button class="primary" onclick={() => install()} disabled={running || !ready}>{running ? "Installing…" : error ? "Try again" : "Install"}</button>
        {/if}
      {/if}
    </div>
  </div>
</div>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 200;
    display: grid;
    place-items: center;
  }
  .dialog {
    width: min(520px, calc(100% - 32px));
    padding: 18px 20px 16px;
  }
  h2 {
    margin: 0 0 8px;
    font-size: 14px;
    font-weight: 600;
    color: var(--jojo-fg);
  }
  .hint {
    margin: 0 0 12px;
    color: var(--jojo-muted);
    font-size: 12px;
  }
  code {
    font-family: var(--jojo-font-mono);
  }
  label {
    display: grid;
    grid-template-columns: 70px 1fr;
    align-items: center;
    gap: 8px;
    margin-bottom: 8px;
    color: var(--jojo-fg2);
  }
  .signin {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 8px;
    color: var(--jojo-fg2);
  }
  .signin > span {
    width: 70px;
  }
  label.choice,
  label.check {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 0;
  }
  label.check {
    align-items: flex-start;
    margin: 4px 0 10px 78px;
    color: var(--jojo-muted);
    font-size: 12px;
  }
  label.check input {
    margin-top: 2px;
  }
  .pair {
    display: grid;
    grid-template-columns: 1fr 70px;
    gap: 6px;
  }
  input,
  select {
    padding: 5px 8px;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    background: var(--jojo-pane);
    color: var(--jojo-fg);
    font: inherit;
  }
  .steps {
    margin: 12px 0 0;
    padding: 0;
    list-style: none;
  }
  .steps li {
    display: grid;
    grid-template-columns: 16px 130px 1fr;
    gap: 6px;
    padding: 3px 0;
    color: var(--jojo-fg2);
    font-size: 12px;
  }
  .steps .ok .mark {
    color: var(--jojo-done);
  }
  .steps .warning .mark,
  .steps .warning .message {
    color: var(--jojo-needs-you);
  }
  .steps .failed .mark,
  .steps .failed .message,
  .error {
    color: var(--jojo-error);
  }
  .error {
    margin: 10px 0 0;
    font-size: 12px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 14px;
  }
  .actions button {
    padding: 5px 12px;
    border: var(--jojo-bw) solid var(--jojo-line);
    border-radius: var(--jojo-radius-sm);
    color: var(--jojo-fg2);
  }
  .actions .primary {
    border-color: var(--jojo-accent);
    background: var(--jojo-accent);
    color: var(--jojo-bg);
  }
  .actions button:disabled {
    opacity: 0.5;
  }
</style>
