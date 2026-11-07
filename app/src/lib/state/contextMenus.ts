// The pane, tab and sidebar session menus (spec 9.4), bound to app actions.
import { tabLabel, tabOf } from "../layout/tabs";
import { buildPaneMenu, buildSessionMenu, buildTabMenu } from "../menu/menus";
import type { MenuEntry } from "../menu/types";
import { terminalControls } from "../terminal/registry";
import {
  app,
  applyTabPreset,
  closePane,
  closeTab,
  currentProjectLayout,
  currentTab,
  equalizeTabSizes,
  killSession,
  moveSessionToNewTab,
  moveSessionToTab,
  openSessionInSplit,
  projectLayout,
  restartSession,
  splitSession,
  swapSessions,
  zoomSession,
} from "./app.svelte";
import { copyFromPane, pasteIntoPane } from "./clipboard";
import { shellChoices } from "./shells";
import { beginRename } from "./ui.svelte";

const labelOf = (sessionId: string): string | undefined => app.model.sessions[sessionId]?.label;

export function paneMenu(sessionId: string): MenuEntry[] {
  const projectId = app.model.sessions[sessionId]?.project_id;
  if (!projectId) return [];
  const project = projectLayout(projectId);
  const tab = tabOf(project, sessionId);
  if (!tab) return [];
  return buildPaneMenu(
    {
      sessionId,
      hasSelection: terminalControls(sessionId)?.hasSelection() ?? false,
      tab,
      otherTabs: project.tabs.filter((t) => t.id !== tab.id),
      labelOf: (id) => labelOf(id) ?? id,
      tabLabelOf: (t) => tabLabel(t, labelOf),
      shells: shellChoices(app.shells, app.settings?.defaultShell),
    },
    {
      copy: () => void copyFromPane(sessionId),
      paste: () => void pasteIntoPane(sessionId),
      split: (direction, kind, shell) => splitSession(sessionId, direction, kind, shell),
      toggleZoom: () => zoomSession(sessionId),
      swapWith: (other) => swapSessions(sessionId, other),
      moveToNewTab: () => moveSessionToNewTab(sessionId),
      moveToTab: (tabId) => moveSessionToTab(sessionId, tabId),
      applyPreset: (preset) => applyTabPreset(tab.id, preset),
      equalize: () => equalizeTabSizes(tab.id),
      rename: () => beginRename({ kind: "session", sessionId, where: "pane" }),
      close: () => void closePane(sessionId),
    },
  );
}

export function tabMenu(tabId: string): MenuEntry[] {
  const tab = currentProjectLayout()?.tabs.find((t) => t.id === tabId);
  if (!tab) return [];
  return buildTabMenu(tab, {
    rename: () => beginRename({ kind: "tab", tabId }),
    applyPreset: (preset) => applyTabPreset(tabId, preset),
    close: () => void closeTab(tabId),
  });
}

export function sessionMenu(sessionId: string): MenuEntry[] {
  const session = app.model.sessions[sessionId];
  if (!session) return [];
  return buildSessionMenu(
    {
      session,
      currentTab: currentTab(),
      currentProjectId: app.selectedProjectId,
      sessionTab: tabOf(projectLayout(session.project_id), sessionId),
    },
    {
      openInSplit: (direction) => openSessionInSplit(sessionId, direction),
      moveToNewTab: () => moveSessionToNewTab(sessionId),
      rename: () => beginRename({ kind: "session", sessionId, where: "sidebar" }),
      restart: () => restartSession(sessionId),
      kill: () => killSession(sessionId),
    },
  );
}
