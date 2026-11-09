// A short chime when a session finishes or needs a response (user request,
// like Herdr). Synthesized with the Web Audio API, so no sound file ships.
import type { AgentState } from "../protocol/AgentState";

/** At most one chime per session in this many milliseconds. */
export const CHIME_INTERVAL_MS = 10_000;

/** Notes (Hz) per state: rising for done, a repeated higher pair for needs you, falling for error. */
const NOTES: Partial<Record<AgentState, number[]>> = {
  done: [659.25, 987.77],
  needs_you: [880, 1174.66, 880, 1174.66],
  error: [523.25, 392],
};

/**
 * Whether a state change chimes: the session newly entered done, needs_you
 * or error, sound is on, and it has not chimed in the last
 * CHIME_INTERVAL_MS. Unlike toasts, it chimes for the focused pane too: a
 * finished turn is worth hearing while looking elsewhere in the same window.
 */
export function chimeFor(
  previous: AgentState | undefined,
  next: AgentState,
  enabled: boolean,
  lastChimeAt: number | undefined,
  now: number,
): AgentState | null {
  if (!enabled || previous === next || !NOTES[next]) return null;
  if (lastChimeAt !== undefined && now - lastChimeAt < CHIME_INTERVAL_MS) return null;
  return next;
}

let context: AudioContext | null = null;

/** Plays the chime for `state` (quiet sine notes, 120 ms apart). */
export function playChime(state: AgentState): void {
  const notes = NOTES[state];
  if (!notes || typeof AudioContext === "undefined") return;
  context ??= new AudioContext();
  const start = context.currentTime + 0.02;
  notes.forEach((frequency, i) => {
    const at = start + i * 0.12;
    const oscillator = context!.createOscillator();
    const gain = context!.createGain();
    oscillator.type = "sine";
    oscillator.frequency.value = frequency;
    // A soft attack and a quick decay, at a volume below a notification sound.
    gain.gain.setValueAtTime(0, at);
    gain.gain.linearRampToValueAtTime(0.18, at + 0.015);
    gain.gain.exponentialRampToValueAtTime(0.0001, at + 0.28);
    oscillator.connect(gain).connect(context!.destination);
    oscillator.start(at);
    oscillator.stop(at + 0.3);
  });
}
