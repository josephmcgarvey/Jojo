import { describe, expect, it } from "vitest";
import { connectionBanner } from "./banner";

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
      text: "jojod speaks protocol 1.",
      action: "Restart jojod (ends all running sessions)",
    });
  });
});
