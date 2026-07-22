// v0.9.2 +S2 — State Time-travel (SDK side).
//
// `sentori.bindState({ redux, zustand })` subscribes to a store-like
// object and records a shallow diff every time it emits a change.
// A ring buffer of the last N snapshots travels with each
// captureException as a `stateSnapshot` attachment — the dashboard's
// time-travel viewer (v0.9.2.1) renders the timeline.
//
// Why diffs and not full snapshots:
//   • prod stores can be huge (carts, paginated lists, etc.)
//   • a diff drops to ~1% of full snapshot size in typical apps
//   • the viewer rehydrates by applying diffs forward from a baseline
//
// `recordState(obj)` is the manual escape hatch for state that isn't
// in a redux/zustand store (e.g. `useState` in a deeply-nested
// component, or React Context). The host calls it where it makes sense.
//
// Privacy: the same mask query that protects screenshots is consulted
// before serializing — any path matching `nativeID` shape isn't yet,
// but `mask.matchPath` can be added later. v0.9.2 ships without
// path masking; consumers should keep PII out of bindState scopes.

const MAX_SNAPSHOTS = 50;

type StoreLike = {
  getState?: () => unknown;
  subscribe?: (cb: () => void) => () => void;
};

export type StateSnapshot = {
  /** Wall-clock when the diff fired. */
  ts: number;
  /** Top-level key/value diff vs the previous snapshot. Empty diff
   *  doesn't get recorded. */
  diff: Record<string, unknown>;
  /** Source label so the viewer can show "Redux" / "Zustand" / "Manual". */
  source: string;
};

let _snapshots: StateSnapshot[] = [];
let _unsubscribers: (() => void)[] = [];
let _lastByLabel: Record<string, unknown> = {};

export function bindState(opts: {
  redux?: StoreLike;
  zustand?: StoreLike;
  /** Additional named stores. The label is used as the snapshot's
   *  `source` and as the diff bucket key. */
  custom?: Record<string, StoreLike>;
}): void {
  unbindState();
  bindOne('redux', opts.redux);
  bindOne('zustand', opts.zustand);
  if (opts.custom) {
    for (const [label, store] of Object.entries(opts.custom)) {
      bindOne(label, store);
    }
  }
}

function bindOne(label: string, store?: StoreLike): void {
  if (!store) return;
  if (typeof store.getState !== 'function' || typeof store.subscribe !== 'function') return;
  try {
    _lastByLabel[label] = store.getState();
    const unsub = store.subscribe(() => {
      const next = store.getState!();
      const prev = _lastByLabel[label];
      const diff = shallowDiff(prev, next);
      if (diff && Object.keys(diff).length > 0) {
        push({ diff, source: label, ts: Date.now() });
      }
      _lastByLabel[label] = next;
    });
    _unsubscribers.push(unsub);
  } catch {
    // ignore bad stores
  }
}

export function unbindState(): void {
  for (const u of _unsubscribers) {
    try {
      u();
    } catch {
      // ignore
    }
  }
  _unsubscribers = [];
  _lastByLabel = {};
}

/** Manual recording for state not in a subscribed store. */
export function recordState(snapshot: Record<string, unknown>, source = 'manual'): void {
  push({ diff: snapshot, source, ts: Date.now() });
}

export function getStateSnapshots(): StateSnapshot[] {
  return _snapshots.slice();
}

export function clearStateSnapshots(): void {
  _snapshots = [];
}

function push(s: StateSnapshot): void {
  _snapshots.push(s);
  while (_snapshots.length > MAX_SNAPSHOTS) _snapshots.shift();
}

/** Returns the top-level key/value diff (next has the value, prev
 *  may not contain the key). Empty diff returns `{}` so callers can
 *  no-op on empty. */
export function shallowDiff(prev: unknown, next: unknown): null | Record<string, unknown> {
  if (typeof next !== 'object' || next === null) return null;
  const nObj = next as Record<string, unknown>;
  if (typeof prev !== 'object' || prev === null) {
    return { ...nObj };
  }
  const pObj = prev as Record<string, unknown>;
  const out: Record<string, unknown> = {};
  const keys = new Set([...Object.keys(pObj), ...Object.keys(nObj)]);
  for (const k of keys) {
    if (pObj[k] !== nObj[k]) out[k] = nObj[k];
  }
  return out;
}

/** Test-only. */
export function __resetStateSnapshotsForTests(): void {
  unbindState();
  _snapshots = [];
}
