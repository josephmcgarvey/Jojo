import { describe, expect, it } from "vitest";
import { bytesToUlid, newUlid, ulidToBytes } from "./ulid";

describe("ulid", () => {
  it("encodes the zero and maximum values", () => {
    expect(bytesToUlid(new Uint8Array(16))).toBe("00000000000000000000000000");
    expect(bytesToUlid(new Uint8Array(16).fill(0xff))).toBe("7ZZZZZZZZZZZZZZZZZZZZZZZZZ");
  });

  it("round-trips random values", () => {
    for (let i = 0; i < 100; i++) {
      const id = newUlid();
      expect(id).toMatch(/^[0-9A-HJKMNP-TV-Z]{26}$/);
      expect(bytesToUlid(ulidToBytes(id))).toBe(id);
    }
  });

  it("puts the timestamp in the first 48 bits", () => {
    const before = Date.now();
    const bytes = ulidToBytes(newUlid());
    const ms = bytes.slice(0, 6).reduce((acc, b) => acc * 256 + b, 0);
    expect(ms).toBeGreaterThanOrEqual(before);
    expect(ms).toBeLessThanOrEqual(Date.now());
  });

  it("rejects malformed ids", () => {
    expect(() => ulidToBytes("short")).toThrow();
    expect(() => ulidToBytes("0000000000000000000000000U")).toThrow();
  });
});
