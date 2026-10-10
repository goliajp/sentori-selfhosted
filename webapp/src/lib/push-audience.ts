import type { AudienceRequest } from './api';
import type { MessageKey } from '../i18n/en';
export type Source = 'device' | 'issue' | 'trait' | 'user';
export type Op =
  | 'exists'
  | 'gte'
  | 'in'
  | 'is'
  | 'isNot'
  | 'lte'
  | 'prefix'
  | 'versionGte'
  | 'versionLte';

export type Condition = { key: string; op: Op; source: Source; value: string };

export const OPS: { label: MessageKey; value: Op }[] = [
  { label: 'push.opIs', value: 'is' },
  { label: 'push.opIsNot', value: 'isNot' },
  { label: 'push.opIn', value: 'in' },
  { label: 'push.opPrefix', value: 'prefix' },
  { label: 'push.opExists', value: 'exists' },
  { label: 'push.opVersionGte', value: 'versionGte' },
  { label: 'push.opVersionLte', value: 'versionLte' },
  { label: 'push.opGte', value: 'gte' },
  { label: 'push.opLte', value: 'lte' },
];

export const blank = (): Condition => ({
  key: '',
  op: 'is',
  source: 'trait',
  value: '',
});

/// A key for one send. `crypto.randomUUID` needs a secure context,
/// which the dashboard is; the fallback is for the one that is not.
export const valueOnly = (r: Condition) =>
  r.source === 'user' || r.source === 'issue';

/// A written value, as the type the server compares with.
///
/// `"true"` and `"42"` come out of a text field as strings, and a
/// trait stored as a boolean would then never match one. Version and
/// prefix comparisons are the exception — those are strings by
/// definition, and "4.20" must not become the number 4.2.
export function typed(op: Op, raw: string): unknown {
  const s = raw.trim();
  if (op === 'prefix' || op === 'versionGte' || op === 'versionLte') return s;
  if (s === 'true') return true;
  if (s === 'false') return false;
  if (s === 'null') return null;
  if (s !== '' && !Number.isNaN(Number(s))) return Number(s);
  return s;
}

/// The rows as the server's shape, or null when a row is unfinished.
///
/// Null rather than a partial expression: a half-written condition
/// that quietly compiles matches something nobody asked for, and the
/// count next to it would look like an answer.
export function toRequest(
  join: 'all' | 'any',
  rows: Condition[],
): AudienceRequest | null {
  const usable = rows.filter((r) =>
    valueOnly(r) ? r.value.trim() : r.key.trim(),
  );
  if (usable.length === 0) return null;

  const leaves = usable.map((r) => {
    if (r.source === 'user') return { user: r.value.trim() };
    // Everyone an issue happened to. The server joins it against the
    // same identity hash the events carry, so this is the condition
    // behind "tell the people who hit this that it is fixed".
    if (r.source === 'issue') return { issue: r.value.trim() };
    const where =
      r.source === 'trait' ? { trait: r.key.trim() } : { device: r.key.trim() };
    if (r.op === 'exists') return { ...where, exists: true };
    if (r.op === 'in') {
      return {
        ...where,
        in: r.value
          .split(',')
          .map((v) => v.trim())
          .filter((v) => v !== '')
          .map((v) => typed('is', v)),
      };
    }
    return { ...where, [r.op]: typed(r.op, r.value) };
  });

  return {
    audience:
      leaves.length === 1 && join === 'all' ? leaves[0] : { [join]: leaves },
  };
}
