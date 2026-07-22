// v0.9.0 #12 — pure rage-tap detection logic. Lives outside the .tsx
// component so unit tests can import it without dragging in
// `react-native` (whose flow syntax breaks bun:test parser).

export const RAGE_WINDOW_MS = 800;
export const RAGE_THRESHOLD = 3;

/** Given the per-target recent-tap buckets, a target id, and `now`,
 *  return `true` iff this tap crosses the rage threshold. Side
 *  effect: writes/clears the bucket inside `map` so successive
 *  taps after a triggered rage event don't immediately re-trigger. */
export function recordTap(
  map: Map<number, number[]>,
  target: number,
  now: number,
): boolean {
  const previous = map.get(target) ?? [];
  const fresh = previous.filter((t) => now - t <= RAGE_WINDOW_MS);
  fresh.push(now);
  if (fresh.length >= RAGE_THRESHOLD) {
    map.delete(target);
    return true;
  }
  map.set(target, fresh);
  return false;
}
