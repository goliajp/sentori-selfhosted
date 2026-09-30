// Sessions — the denominator.
//
// The five kinds count what went wrong. Nothing counted what went
// right, so "18 errors" had nothing to divide by and the first number
// a mobile team is asked for — the share of sessions that did not
// crash — could not be computed at all.
//
// `SessionTracker` has been in `@goliapkg/sentori-core` since phase 26
// with no callers. This is the wiring.
//
// ## How a crash is recognised
//
// A session that ends cleanly sends `exited`. A session that never
// ends leaves its record on disk, and the next launch finds it. That
// alone does not make it a crash: the OS reclaiming memory and the
// user swiping the app away look identical from here, and counting
// those as crashes would understate the rate for a reason that is not
// the app's fault.
//
// So the recovered session is `crashed` only when the native handler
// also left a crash file this launch, and `exited` otherwise. That is
// the same evidence the crash report itself rests on.

import type { SessionContext, SessionPing } from '@goliapkg/sentori-core';
import { SessionTracker, uuidV7 } from '@goliapkg/sentori-core';

import { currentUserKey, onIdentityChange } from './scope';
import { queueSession } from './transport';

const KEY = '@sentori/session';

type Persisted = {
  ctx: SessionContext;
  id: string;
  startedAtMs: number;
  /** Who was signed in when this launch was last seen alive. A crashed
   *  session belongs to that person, not to whoever the next launch
   *  signs in — so it is written here rather than read live on
   *  recovery. */
  userKey?: null | string;
};

type StorageLike = {
  getItem(key: string): Promise<null | string>;
  removeItem(key: string): Promise<void>;
  setItem(key: string, value: string): Promise<void>;
};

let _storage: null | StorageLike = null;
let _tracker: null | SessionTracker = null;
let _ctx: null | SessionContext = null;

/** Injected by `init`; separate so tests can drive it without RN. */
export const __setStorageForTests = (storage: null | StorageLike): void => {
  _storage = storage;
};

const storage = (): null | StorageLike => {
  if (_storage) return _storage;
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const mod = require('@react-native-async-storage/async-storage') as {
      default?: StorageLike;
    };
    _storage = mod.default ?? null;
  } catch {
    // No storage means no crash recovery: a session that ends cleanly
    // still counts, so the rate is high rather than absent.
    _storage = null;
  }
  return _storage;
};

/**
 * Begin a session and remember it, so a launch that never ends it can
 * be recognised as an abnormal exit.
 */
export const startSession = (ctx: SessionContext): void => {
  _ctx = ctx;
  // Read at send time, not at start. Apps learn who the user is after
  // launch — a login screen guarantees it — so a key captured here is
  // null for every session that matters.
  _tracker ??= new SessionTracker((ping: SessionPing) =>
    queueSession({ ...ping, userKey: currentUserKey() ?? null }),
  );
  _tracker.start(ctx);
  _record = { ctx, id: uuidV7(), startedAtMs: Date.now(), userKey: currentUserKey() ?? null };
  persist();
  // And again whenever it changes, so the record on disk names the
  // person a crash would be attributed to.
  onIdentityChange(rememberUser);
};

let _record: null | Persisted = null;

const persist = (): void => {
  if (!_record) return;
  void storage()
    ?.setItem(KEY, JSON.stringify(_record))
    .catch(() => undefined);
};

const rememberUser = (): void => {
  if (!_record) return;
  const key = currentUserKey() ?? null;
  if (_record.userKey === key) return;
  _record.userKey = key;
  persist();
};

/** A non-fatal error happened during this session. */
export const markSessionErrored = (): void => {
  _tracker?.markErrored();
};

/** End it cleanly. The record goes with it. */
export const endSession = (): void => {
  _tracker?.end('exited');
  _record = null;
  void storage()
    ?.removeItem(KEY)
    .catch(() => undefined);
};

/**
 * Close out a session the last launch left open.
 *
 * `crashedThisLaunch` is whether the native handler left a crash file
 * — the difference between "the app died of a bug" and "the OS or the
 * user closed it", which the rate would otherwise conflate.
 */
export const recoverSession = async (crashedThisLaunch: boolean): Promise<void> => {
  const store = storage();
  if (!store) return;
  let raw: null | string = null;
  try {
    raw = await store.getItem(KEY);
  } catch {
    return;
  }
  if (!raw) return;
  try {
    await store.removeItem(KEY);
  } catch {
    // If it cannot be cleared, sending anyway would report the same
    // session on every launch forever. Stop instead.
    return;
  }
  try {
    const record = JSON.parse(raw) as Persisted;
    if (!record?.ctx || typeof record.startedAtMs !== 'number') return;
    // The id has to be a uuid: the server's column is one, and a
    // malformed session must never be able to cost the events
    // travelling in the same envelope.
    if (typeof record.id !== 'string' || record.id.length !== 36) return;
    queueSession({
      durationMs: Math.max(0, Date.now() - record.startedAtMs),
      environment: record.ctx.environment,
      id: record.id,
      platform: record.ctx.platform,
      release: record.ctx.release,
      startedAt: new Date(record.startedAtMs).toISOString(),
      status: crashedThisLaunch ? 'crashed' : 'exited',
      userId: record.ctx.userId,
      userKey: record.userKey ?? null,
    });
  } catch {
    // A record we cannot read is one we cannot count. It is already
    // removed, so this does not repeat.
  }
};

export const __resetSessionsForTests = (): void => {
  _tracker = null;
  _ctx = null;
  _storage = null;
  _record = null;
};
