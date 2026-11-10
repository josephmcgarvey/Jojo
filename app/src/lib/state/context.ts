// The context meter on a Claude session's sidebar row. jojod reads how full
// the conversation's context window is from its transcript after each hook
// event (crates/jojo-agent/src/context.rs); this module turns the numbers
// into the meter's fill, color level and tooltip.

/** At or above this fraction of the window the meter turns amber: time to plan a /compact. */
export const WARN_AT = 0.7;
/** At or above this fraction it turns red: Claude Code will compact on its own soon. */
export const FULL_AT = 0.85;

export type ContextLevel = "ok" | "warn" | "full";

export interface ContextMeter {
  /** The used fraction, clamped to 0 to 1. */
  fraction: number;
  level: ContextLevel;
  /** "150k of 200k tokens (75%)". */
  text: string;
}

/** 999 → "999", 150004 → "150k", 1000000 → "1M", 1250000 → "1.3M". */
export function formatTokens(tokens: number): string {
  if (tokens >= 1_000_000) return `${+(tokens / 1_000_000).toFixed(1)}M`;
  if (tokens >= 1_000) return `${Math.round(tokens / 1_000)}k`;
  return String(tokens);
}

/** The meter for a session, or null when jojod has no usage for it (not a Claude conversation, or no reply yet). */
export function contextMeter(tokens: number | null, window: number | null): ContextMeter | null {
  if (tokens === null || window === null || window <= 0) return null;
  const fraction = Math.min(1, Math.max(0, tokens / window));
  const level: ContextLevel = fraction >= FULL_AT ? "full" : fraction >= WARN_AT ? "warn" : "ok";
  return { fraction, level, text: `${formatTokens(tokens)} of ${formatTokens(window)} tokens (${Math.round(fraction * 100)}%)` };
}

/** The meter's tooltip. */
export function contextTooltip(meter: ContextMeter): string {
  const advice =
    meter.level === "full"
      ? "Run /compact now; Claude Code compacts on its own when the window is nearly full."
      : meter.level === "warn"
        ? "Consider running /compact at a good stopping point."
        : "Plenty of room.";
  return `Context: ${meter.text}. ${advice}`;
}
