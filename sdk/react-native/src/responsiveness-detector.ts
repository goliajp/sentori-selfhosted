// dead_button / sluggish_button — pure detection logic.
//
// A tap is judged by what the signal ring records after it: any
// non-tap signal (navigation, network, a trace point) inside the
// response window counts as the app reacting.
//
//   - responsive: a reaction within SLUGGISH_MS.
//   - sluggish:   a reaction, but slower than SLUGGISH_MS.
//   - dead:       no reaction at all inside RESPONSE_WINDOW_MS.
//
// One dead tap is usually decoration (backgrounds, labels), so a
// dead_button warn needs DEAD_THRESHOLD dead taps on the SAME
// target inside DEAD_WINDOW_MS — a user repeatedly poking one
// unresponsive control, at a slower cadence than a rage tap.
// Sluggish warns are per-target cooldown-limited so one slow
// button files one issue, not one per tap.

export const RESPONSE_WINDOW_MS = 1_500;
export const SLUGGISH_MS = 1_000;
export const DEAD_THRESHOLD = 3;
export const DEAD_WINDOW_MS = 30_000;
export const SLUGGISH_COOLDOWN_MS = 60_000;

export type TapOutcome = 'dead' | 'responsive' | 'sluggish';

/** Judge one tap by the ring signals that followed it.
 *  `signalTimes` are the timestamps (ms) of every non-tap signal
 *  recorded after `tapAt`. */
export function classifyTap(tapAt: number, signalTimes: number[]): TapOutcome {
  const first = signalTimes
    .filter((t) => t > tapAt && t - tapAt <= RESPONSE_WINDOW_MS)
    .sort((a, b) => a - b)[0];
  if (first === undefined) return 'dead';
  return first - tapAt > SLUGGISH_MS ? 'sluggish' : 'responsive';
}

/** Per-target dead-tap bookkeeping. Returns true when this dead tap
 *  crosses the warn threshold (and clears the bucket so the next
 *  warn needs a fresh run of dead taps). */
export function recordDeadTap(
  buckets: Map<number, number[]>,
  target: number,
  now: number,
): boolean {
  const fresh = (buckets.get(target) ?? []).filter((t) => now - t <= DEAD_WINDOW_MS);
  fresh.push(now);
  if (fresh.length >= DEAD_THRESHOLD) {
    buckets.delete(target);
    return true;
  }
  buckets.set(target, fresh);
  return false;
}

/** Per-target sluggish cooldown. Returns true when a warn should
 *  fire (and stamps the cooldown). */
export function recordSluggish(
  lastWarnAt: Map<number, number>,
  target: number,
  now: number,
): boolean {
  const last = lastWarnAt.get(target);
  if (last !== undefined && now - last < SLUGGISH_COOLDOWN_MS) return false;
  lastWarnAt.set(target, now);
  return true;
}
