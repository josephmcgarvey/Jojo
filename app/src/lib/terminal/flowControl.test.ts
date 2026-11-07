import { describe, expect, it } from "vitest";
import { HIGH_WATERMARK, LOW_WATERMARK, WriteFlow } from "./flowControl";

describe("WriteFlow", () => {
  it("reports crossing the high watermark once", () => {
    const flow = new WriteFlow();
    expect(flow.wrote(HIGH_WATERMARK)).toBe(false);
    expect(flow.isBehind).toBe(false);
    expect(flow.wrote(1)).toBe(true);
    expect(flow.isBehind).toBe(true);
    expect(flow.wrote(1)).toBe(false);
    expect(flow.pendingBytes).toBe(HIGH_WATERMARK + 2);
  });

  it("reports catching up once the queue drains to the low watermark", () => {
    const flow = new WriteFlow();
    flow.wrote(HIGH_WATERMARK + 1);
    expect(flow.parsed(HIGH_WATERMARK - LOW_WATERMARK)).toBe(false);
    expect(flow.parsed(1)).toBe(true);
    expect(flow.isBehind).toBe(false);
    expect(flow.parsed(1)).toBe(false);
  });

  it("never reports catching up when it was not behind, and never goes below zero", () => {
    const flow = new WriteFlow();
    flow.wrote(10);
    expect(flow.parsed(10)).toBe(false);
    expect(flow.parsed(10)).toBe(false);
    expect(flow.pendingBytes).toBe(0);
  });

  it("keeps a full-size snapshot below the high watermark", () => {
    expect(HIGH_WATERMARK).toBeGreaterThan(16 * 1024 * 1024);
    expect(HIGH_WATERMARK + 16 * 1024 * 1024).toBeLessThan(50_000_000);
  });
});
