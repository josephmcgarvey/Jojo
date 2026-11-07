<script lang="ts">
  import { onMount } from "svelte";
  import neonGlass from "./themes/neon-glass.json";
  import { app, startApp } from "./lib/state/app.svelte";
  import { applyTheme, type Theme } from "./lib/theme";
  import Banner from "./ui/Banner.svelte";
  import Sidebar from "./ui/Sidebar.svelte";
  import StatusBar from "./ui/StatusBar.svelte";
  import TabBar from "./ui/TabBar.svelte";
  import TerminalPane from "./ui/TerminalPane.svelte";

  const theme = neonGlass as Theme;
  onMount(() => {
    applyTheme(theme);
    return startApp();
  });
  const active = $derived(app.activeSessionId ? app.model.sessions[app.activeSessionId] : undefined);
</script>

<div class="window">
  <Sidebar />
  <section class="main">
    <TabBar />
    <Banner />
    <div class="panes">
      {#if active}
        {#key active.id}
          <TerminalPane session={active} terminalTheme={theme.terminal} />
        {/key}
      {:else if app.selectedProjectId}
        <p class="empty">No session open. Use + to start Claude or a shell.</p>
      {:else}
        <p class="empty">Add a project folder with + next to This PC.</p>
      {/if}
    </div>
    <StatusBar />
  </section>
</div>

<style>
  .window {
    display: grid;
    grid-template-columns: 250px 1fr;
    height: 100%;
    background: var(--jojo-bg-glow);
  }
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .panes {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .empty {
    margin: 40px auto;
    text-align: center;
    color: var(--jojo-muted);
  }
</style>
