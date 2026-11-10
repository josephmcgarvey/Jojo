import { describe, expect, it } from "vitest";
import { connectionBanner, errorBannerText } from "./banner";

const LOG = String.raw`%LOCALAPPDATA%\Jojo\logs\jojod.<date>.log`;

describe("connectionBanner", () => {
  it("shows nothing while connected", () => {
    expect(connectionBanner({ state: "connected", jojod_version: "0.1.0" }, true)).toBeNull();
  });

  it("says jojod is starting until the first connection has succeeded", () => {
    expect(connectionBanner({ state: "starting" }, false)).toEqual({ text: "Starting jojod…", action: null });
    expect(connectionBanner({ state: "reconnecting", attempt: 3, message: "pipe not found" }, false)).toEqual({
      text: "Starting jojod (attempt 3): pipe not found.",
      action: null,
    });
    expect(connectionBanner({ state: "failed", message: "start timed out" }, false)).toEqual({
      text: `jojod did not start: start timed out. Logs are in ${LOG}.`,
      action: "Retry",
    });
  });

  it("says sessions keep running only after a connection was lost", () => {
    expect(connectionBanner({ state: "reconnecting", attempt: 2, message: "pipe closed" }, true)).toEqual({
      text: "Disconnected from jojod: pipe closed. Reconnecting (attempt 2). Sessions keep running.",
      action: null,
    });
    expect(connectionBanner({ state: "failed", message: "pipe closed" }, true)).toEqual({
      text: `Cannot reach jojod: pipe closed. Logs are in ${LOG}.`,
      action: "Retry",
    });
  });

  it("keeps the elevated and version mismatch banners", () => {
    expect(connectionBanner({ state: "elevated" }, false)?.text).toContain("running as administrator");
    expect(connectionBanner({ state: "version_mismatch", message: "jojod speaks protocol 1" }, true)).toEqual({
      text: "jojod speaks protocol 1. Until jojod restarts, its sessions keep running but cannot be shown.",
      action: "Restart now (ends all running sessions)",
      later: true,
    });
    expect(connectionBanner({ state: "version_mismatch", message: "x" }, true, 3)?.action).toBe("Restart now (ends 3 sessions)");
    expect(connectionBanner({ state: "version_mismatch", message: "x" }, true, 1)?.action).toBe("Restart now (ends 1 session)");
  });
});

describe("errorBannerText", () => {
  it("explains an error from a daemon older than the app", () => {
    expect(errorBannerText({ request: "Frame", message: "bad control JSON: unknown variant `RenameProject`, expected one of `Hello`" })).toBe(
      "This jojod is older than the app and does not support RenameProject. On This PC, use Settings ▸ Restart daemon; for a remote location, add it again with Remote location, which installs the new jojod.",
    );
  });

  it("shows other errors as request and message", () => {
    expect(errorBannerText({ request: "AddProject", message: "not a directory: /srv/x" })).toBe("AddProject: not a directory: /srv/x");
  });
});
