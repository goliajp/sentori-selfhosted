// v0.9.0 #3 — launch-crash loop guard.
//
// On every init we write a "launch_marker" to AsyncStorage. On
// `markLaunchCompleted()` we write a sibling "launch_completed". On
// startup we look at the previous launch state: marker present but
// completed missing → previous launch did not finish → increment a
// consecutive-crash counter.
//
// When the counter crosses `threshold` (default 2), we invoke the
// host-supplied `onLaunchCrashDetected` callback with a 200 ms timeout
// (D3) and follow its action: rollback the OTA bundle, reset a list
// of AsyncStorage keys, or continue. Rollback / reset trigger an
// `expo-updates` reload when available.
//
// v0.9.0 scope: JS-only — catches everything that runs after the JS
// bridge is up (almost every OTA-induced launch crash). v0.9.1 will
// add a native marker for the small set of "crashed before bridge"
// cases.

import { isAnyNativeModuleLinked } from './native-loader';

const MARKER_KEY = '@sentori/launch_marker';
const COMPLETED_KEY = '@sentori/launch_completed';
const COUNT_KEY = '@sentori/launch_crash_count';

export type LaunchCrashInfo = {
  /** Consecutive failed launches detected so far (this one inclusive). */
  consecutiveCount: number;
  /** OTA bundle id of the crashing launch, if known. */
  crashedBundle: null | string;
  /** Most recent bundle id that *did* reach `markLaunchCompleted`. */
  lastSafeBundle: null | string;
  /** Store-binary release of the crashing launch. */
  release: string;
};

export type LaunchCrashAction =
  | { action: 'continue' }
  | { action: 'reset'; clearKeys: string[] }
  | { action: 'rollback'; toBundle?: null | string };

export type LaunchCrashGuardOptions = {
  enabled: boolean;
  onLaunchCrashDetected?: (info: LaunchCrashInfo) => LaunchCrashAction | Promise<LaunchCrashAction>;
  /** Default 2 — fires after the second consecutive failed launch. */
  threshold?: number;
  /** Default 200 — D3 decision. */
  timeoutMs?: number;
};

type AsyncStorageLike = {
  getItem: (key: string) => Promise<null | string>;
  multiRemove?: (keys: string[]) => Promise<void>;
  removeItem: (key: string) => Promise<void>;
  setItem: (key: string, value: string) => Promise<void>;
};

function loadAsyncStorage(): AsyncStorageLike | null {
  // v0.8.5 — same NativeModule guard as netinfo/transport. The
  // require() can succeed without the native module linked, but
  // getItem will crash from a microtask path our try/catch can't
  // reach.
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

/** Returns `false` iff we triggered a bundle rollback / reset and
 *  expect the app to reload momentarily; the caller (init) should
 *  short-circuit further setup. */
export async function runLaunchCrashGuard(
  opts: LaunchCrashGuardOptions,
  release: string,
  currentBundleId: null | string,
): Promise<{ shouldContinueInit: boolean; info?: LaunchCrashInfo }> {
  if (!opts.enabled) return { shouldContinueInit: true };
  const storage = loadAsyncStorage();
  if (!storage) return { shouldContinueInit: true };

  try {
    const marker = await storage.getItem(MARKER_KEY);
    const completed = await storage.getItem(COMPLETED_KEY);

    if (marker && !completed) {
      const m = safeJsonParse<{ bundleId?: string; lastSafeBundle?: string }>(marker) ?? {};
      const prevCount = parseInt((await storage.getItem(COUNT_KEY)) ?? '0', 10) || 0;
      const consecutiveCount = prevCount + 1;
      await storage.setItem(COUNT_KEY, String(consecutiveCount));

      if (consecutiveCount >= (opts.threshold ?? 2) && opts.onLaunchCrashDetected) {
        const info: LaunchCrashInfo = {
          consecutiveCount,
          crashedBundle: m.bundleId ?? null,
          lastSafeBundle: m.lastSafeBundle ?? null,
          release,
        };
        const action = await raceWithTimeout<LaunchCrashAction>(
          Promise.resolve(opts.onLaunchCrashDetected(info)),
          opts.timeoutMs ?? 200,
          { action: 'continue' },
        );
        const handled = await applyAction(action, storage);
        if (!handled.shouldContinueInit) {
          return { ...handled, info };
        }
        return { ...handled, info };
      }
    } else {
      // Previous launch completed; clean the counter.
      await storage.setItem(COUNT_KEY, '0');
    }

    // Write the marker for THIS launch. lastSafeBundle = previous
    // completed bundle id, so the user's callback can target it.
    const lastSafeBundle =
      (completed && safeJsonParse<{ bundleId?: string }>(completed)?.bundleId) ?? null;
    await storage.setItem(
      MARKER_KEY,
      JSON.stringify({
        bundleId: currentBundleId,
        lastSafeBundle,
        release,
        ts: Date.now(),
      }),
    );
    await storage.removeItem(COMPLETED_KEY);
  } catch {
    // AsyncStorage glitches must never block init.
  }

  return { shouldContinueInit: true };
}

export async function markLaunchCompleted(currentBundleId: null | string): Promise<void> {
  const storage = loadAsyncStorage();
  if (!storage) return;
  try {
    await storage.setItem(
      COMPLETED_KEY,
      JSON.stringify({ bundleId: currentBundleId, ts: Date.now() }),
    );
    await storage.setItem(COUNT_KEY, '0');
  } catch {
    // ignore
  }
}

async function applyAction(
  action: LaunchCrashAction,
  storage: AsyncStorageLike,
): Promise<{ shouldContinueInit: boolean }> {
  if (action.action === 'continue') return { shouldContinueInit: true };
  if (action.action === 'reset') {
    if (storage.multiRemove && Array.isArray(action.clearKeys)) {
      try {
        await storage.multiRemove(action.clearKeys);
      } catch {
        // ignore
      }
    }
    await reloadOTAIfPossible();
    return { shouldContinueInit: false };
  }
  if (action.action === 'rollback') {
    await reloadOTAIfPossible();
    return { shouldContinueInit: false };
  }
  return { shouldContinueInit: true };
}

async function reloadOTAIfPossible(): Promise<void> {
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const Updates = require('expo-updates') as {
      reloadAsync?: () => Promise<void>;
    };
    if (typeof Updates.reloadAsync === 'function') {
      await Updates.reloadAsync();
    }
  } catch {
    // expo-updates not installed — caller will fall through and
    // continue init; their callback returned `rollback` but we can't
    // perform it without the OTA library. Document accordingly.
  }
}

export function raceWithTimeout<T>(p: Promise<T>, ms: number, fallback: T): Promise<T> {
  return new Promise<T>((resolve) => {
    let done = false;
    const t = setTimeout(() => {
      if (!done) {
        done = true;
        resolve(fallback);
      }
    }, ms);
    p.then(
      (v) => {
        if (!done) {
          done = true;
          clearTimeout(t);
          resolve(v);
        }
      },
      () => {
        if (!done) {
          done = true;
          clearTimeout(t);
          resolve(fallback);
        }
      },
    );
  });
}

function safeJsonParse<T>(s: string): null | T {
  try {
    return JSON.parse(s) as T;
  } catch {
    return null;
  }
}
