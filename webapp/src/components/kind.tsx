// The five-kind color system — one stable hue per kind, used
// everywhere a kind appears (Inbox, detail, Instruments) so the
// palette itself teaches the concept model. The values live in
// index.css as `--s-kind-*` and are re-inked per theme; nothing
// here may name a hex.

import type { IssueSummary } from '../lib/api';

export const kindColor = (kind: IssueSummary['kind']): string =>
  `var(--s-kind-${kind})`;

/** A 12% tint of the kind hue — badge fill, row accents. */
export const kindTint = (kind: IssueSummary['kind'], pct = 12): string =>
  `color-mix(in srgb, var(--s-kind-${kind}) ${pct}%, transparent)`;

export function KindBadge({ kind }: { kind: IssueSummary['kind'] }) {
  return (
    <span
      className="inline-block rounded px-1.5 py-0.5 text-xs font-medium"
      style={{ backgroundColor: kindTint(kind), color: kindColor(kind) }}
    >
      {kind}
    </span>
  );
}

export function RegressedBadge() {
  return (
    <span className="inline-block rounded bg-kind-error px-1.5 py-0.5 text-xs font-semibold text-white">
      REGRESSED
    </span>
  );
}

/** breadth × depth, the objective importance pair.
 *  Events without a userKey (pre-identify boot signals) make the
 *  breadth zero — "0u×0" is noise, so the pair collapses to the
 *  event count alone. */
export function ImpactCell({
  users,
  maxPerUser,
  events,
}: {
  users: number;
  maxPerUser: number;
  events: number;
}) {
  return (
    <span className="text-xs tabular-nums text-fg-muted">
      {users > 0 ? `${users}u×${maxPerUser} · ` : ''}
      {events}ev
    </span>
  );
}
