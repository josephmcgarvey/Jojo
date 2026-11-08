// Runs an application shortcut (spec 9.6) against the current state.
import type { AppAction } from "../input/shortcuts";
import { app, closePane, cycleTab, focusDirection, focusedSessionId, newSession, splitFocused, zoomSession } from "./app.svelte";
import { copyFromPane, pasteIntoPane } from "./clipboard";

export function runAction(action: AppAction): void {
  const focused = focusedSessionId();
  switch (action) {
    case "new-claude-tab":
      if (app.selectedProjectId) newSession(app.selectedProjectId, "claude");
      return;
    case "new-shell-tab":
      if (app.selectedProjectId) newSession(app.selectedProjectId, "shell");
      return;
    case "split-right":
      splitFocused("row", "claude");
      return;
    case "split-down":
      splitFocused("column", "claude");
      return;
    case "zoom":
      if (focused) zoomSession(focused);
      return;
    case "focus-left":
      focusDirection("left");
      return;
    case "focus-right":
      focusDirection("right");
      return;
    case "focus-up":
      focusDirection("up");
      return;
    case "focus-down":
      focusDirection("down");
      return;
    case "next-tab":
      cycleTab(1);
      return;
    case "previous-tab":
      cycleTab(-1);
      return;
    case "close-pane":
      if (focused) void closePane(focused);
      return;
    case "copy":
      if (focused) void copyFromPane(focused);
      return;
    case "paste":
      if (focused) void pasteIntoPane(focused);
      return;
  }
}
