import { describe, expect, it } from "vitest";
import { disconnectedOverlay, machineStatusText } from "./machines";

const local = { kind: "local" as const, label: "This PC" };
const server = { kind: "ssh" as const, label: "onprem-server" };

describe("machineStatusText", () => {
  it("shows nothing when connected and a short state otherwise", () => {
    expect(machineStatusText(server, { state: "connected", jojod_version: "0.1.0" })).toBeNull();
    expect(machineStatusText(server, undefined)).toBe("connecting");
    expect(machineStatusText(server, { state: "reconnecting", attempt: 2, message: "x" })).toBe("reconnecting");
    expect(machineStatusText(server, { state: "auth_failed", message: "x" })).toBe("auth failed");
    expect(machineStatusText(server, { state: "unsupported", message: "x" })).toBe("cannot keep sessions");
    expect(machineStatusText(server, { state: "version_mismatch", message: "x" })).toBe("upgrade required");
    expect(machineStatusText(local, { state: "version_mismatch", message: "x" })).toBe("restart required");
  });
});

describe("disconnectedOverlay", () => {
  it("covers a disconnected remote machine's panes only", () => {
    expect(disconnectedOverlay(server, { state: "reconnecting", attempt: 1, message: "x" })).toBe(
      "Disconnected. Sessions keep running on onprem-server.",
    );
    expect(disconnectedOverlay(server, { state: "auth_failed", message: "x" })).toContain("ssh-add");
    expect(disconnectedOverlay(server, { state: "unsupported", message: "web1 runs each SSH connection in its own temporary container." })).toContain("temporary container");
    expect(disconnectedOverlay(server, { state: "connected", jojod_version: "0.1.0" })).toBeNull();
    expect(disconnectedOverlay(local, { state: "reconnecting", attempt: 1, message: "x" })).toBeNull();
  });
});
