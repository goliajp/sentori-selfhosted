// v1.1 chunk S1 — stable per-install identifier.
//
// `getInstallId()` returns a UUIDv7 generated on first call and
// persisted to device storage. The id survives app restarts and, on
// iOS specifically, app reinstalls (because the Keychain backend is
// preserved across uninstall — that's the whole point).
//
// Storage tier order:
//   1. `react-native-keychain` (optional peer; iOS + Android Keystore)
//   2. AsyncStorage (host already needs this for launch-crash-guard /
//      offline queue, so we don't add a hard new peer dep)
//   3. Process-memory only (no persistence — covers SSR / tests)
//
// Opaque to the host. The id is NOT tied to a `setUser` call; the
// server doesn't auto-correlate to user identity. It exists purely
// as a stable device key so the security posture engine (S3) can
// score per-install signals independently of authentication state.

import { uuidV7 } from '@goliapkg/sentori-core';

import { isAnyNativeModuleLinked } from './native-loader';

const KEYCHAIN_SERVICE = 'sentori.install-id';
const ASYNC_STORAGE_KEY = '@sentori/install-id';

type AsyncStorageLike = {
  getItem: (key: string) => Promise<null | string>;
  setItem: (key: string, value: string) => Promise<void>;
};

type KeychainModule = {
  getGenericPassword: (options: {
    service: string;
  }) => Promise<false | { password: string; username: string }>;
  setGenericPassword: (
    username: string,
    password: string,
    options: { service: string },
  ) => Promise<unknown>;
};

let _cached: null | string = null;
let _inflight: null | Promise<string> = null;

function loadKeychain(): KeychainModule | null {
  // react-native-keychain is the recommended secure-storage peer.
  // Optional: if the host hasn't installed it we fall through to
  // AsyncStorage. The Keychain backend is what gives the iOS
  // "survives reinstall" guarantee.
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const mod = require('react-native-keychain') as KeychainModule;
    if (typeof mod.getGenericPassword !== 'function') return null;
    return mod;
  } catch {
    return null;
  }
}

function loadAsyncStorage(): AsyncStorageLike | null {
  if (!isAnyNativeModuleLinked(['RNCAsyncStorage', 'AsyncStorageModule'])) {
    return null;
  }
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const mod = require('@react-native-async-storage/async-storage') as {
      default?: AsyncStorageLike;
    };
    return mod.default ?? (mod as unknown as AsyncStorageLike);
  } catch {
    return null;
  }
}

/**
 * Resolve the device's stable install id, generating one if absent.
 * Cached in memory after first resolve so subsequent calls are sync-
 * adjacent (no storage I/O). Idempotent under concurrent calls — a
 * second caller during the first resolve awaits the same promise.
 */
export async function getInstallId(): Promise<string> {
  if (_cached !== null) return _cached;
  if (_inflight !== null) return _inflight;
  _inflight = (async () => {
    try {
      const kc = loadKeychain();
      if (kc) {
        const existing = await kc
          .getGenericPassword({ service: KEYCHAIN_SERVICE })
          .catch(() => false as const);
        if (existing && existing.password) {
          _cached = existing.password;
          return existing.password;
        }
        const fresh = uuidV7();
        await kc
          .setGenericPassword('sentori', fresh, { service: KEYCHAIN_SERVICE })
          .catch(() => undefined);
        _cached = fresh;
        return fresh;
      }
      const storage = loadAsyncStorage();
      if (storage) {
        const existing = await storage
          .getItem(ASYNC_STORAGE_KEY)
          .catch(() => null);
        if (existing) {
          _cached = existing;
          return existing;
        }
        const fresh = uuidV7();
        await storage.setItem(ASYNC_STORAGE_KEY, fresh).catch(() => undefined);
        _cached = fresh;
        return fresh;
      }
      // No storage available — generate but don't persist. The id is
      // still stable for the lifetime of the process which is good
      // enough for tests / SSR / no-native-modules contexts.
      const fresh = uuidV7();
      _cached = fresh;
      return fresh;
    } finally {
      _inflight = null;
    }
  })();
  return _inflight;
}

/** Sync read of the currently-cached install id. `null` before the
 *  first `getInstallId()` resolves. Use this in hot paths (event
 *  payload assembly) that can't await storage; callers should kick
 *  off `getInstallId()` once at startup to warm the cache. */
export function peekInstallId(): null | string {
  return _cached;
}

export function __resetInstallIdForTests(): void {
  _cached = null;
  _inflight = null;
}

/** For tests + advanced operator flows that need to seed an id from
 *  an external secure-storage migration. */
export function __setInstallIdForTests(id: string): void {
  _cached = id;
}
