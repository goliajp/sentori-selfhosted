// What a release row says before anyone expands it.
//
// The row is four dots. They are the whole screen — a release page
// nobody expands still has to be true at a glance, because the person
// scanning it is checking whether tonight's crash will symbolicate,
// not auditing a file list.
//
// The state that was missing: a kind can hold a good artifact and a
// broken one at the same time. `focus-ai-app@5.4.26081201+383` had a
// working `insight-android-bundle.map` next to an `index.android.bundle`
// somebody had uploaded believing it did something. The good one lit
// the dot green; the bundle said nothing until the row was opened.
// Both facts are true and only one of them was on screen.

/** The four things a release can carry, as the row shows them. */
export type LightState =
  /** Nothing loaded yet. */
  | 'unknown'
  /** Covered, and everything under this kind parses. */
  | 'ok'
  /** Covered, and something under this kind does not parse. */
  | 'broken'
  /** Not covered, and this release needs it. */
  | 'missing'
  /** Not covered, and nothing here uses it. */
  | 'unused';

/**
 * Decide one dot.
 *
 * `on` — is there at least one artifact of this kind the reader can
 * use. `used` — does this release report from a platform that needs
 * it. `broken` — does this kind hold at least one artifact the server
 * stored and could not parse.
 */
export function lightState({
  broken,
  on,
  used,
}: {
  broken: boolean;
  on: boolean | undefined;
  used: boolean;
}): LightState {
  if (on === undefined) return 'unknown';
  if (on) return broken ? 'broken' : 'ok';
  // Nothing usable AND something unreadable is the worst of the two:
  // somebody uploaded for this kind and got nothing, which is a
  // stronger claim on attention than never having uploaded at all.
  if (broken) return 'missing';
  return used ? 'missing' : 'unused';
}

/** The CSS colour for a state. Amber is not decoration — it is the
 *  one state that used to render as green. */
export function lightColour(state: LightState): string {
  const quiet = 'color-mix(in srgb, var(--sn-fg-muted) 30%, transparent)';
  switch (state) {
    case 'ok':
      return 'var(--s-kind-probe)';
    case 'broken':
      return 'var(--s-kind-warn)';
    case 'missing':
      return 'var(--s-kind-error)';
    default:
      return quiet;
  }
}
