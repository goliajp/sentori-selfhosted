// v1.1 chunk S3 — `sentori.queryTrustScore()` for the host app.
//
// Returns the device's current trust score (0–100, higher = healthier)
// plus the per-kind signal mix that produced it. The host app uses
// this to decide whether to step up authentication, decline a
// purchase, or simply log a soft warning.
//
// Caching:
//   L1 — process-memory, ~30 s. Same process should not re-fetch on
//        every screen render; the score moves on a "minutes" cadence.
//   L2 — install-id-derived AsyncStorage entry, ~5 minutes. Lets the
//        host paint a cached score on cold start without waiting for
//        the network. Stale entries are still served; the SDK
//        re-fetches in the background.
//
// Fail-soft posture: if the network's down or the SDK isn't
// initialised, returns the cached value (if any), else the safe
// default (score = 100). The host should never see a thrown error
// from this call — the score system is supposed to be invisible.

import { isAnyNativeModuleLinked } from './native-loader';
import { getConfig, isInitialized } from './config';
import { getInstallId } from './install-id';

const SDK_VERSION = '0.0.0';
const STORAGE_KEY = '@sentori/trust-score';
const L1_TTL_MS = 30_000;
const L2_TTL_MS = 5 * 60_000;

export type TrustSignal = {
  count: number;
  kind: string;
  weight: number;
};

export type TrustScore = {
  computedAt: string;
  installId: string;
  score: number;
  signals: TrustSignal[];
};

type CacheEntry = { fetchedAt: number; score: TrustScore };

type AsyncStorageLike = {
  getItem: (key: string) => Promise<null | string>;
  setItem: (key: string, value: string) => Promise<void>;
};

let _l1: null | CacheEntry = null;
let _inflight: null | Promise<TrustScore> = null;

function loadAsyncStorage(): AsyncStorageLike | null {
  if (!isAnyNativeModuleLinked(['RNCAsyncStorage', 'AsyncStorageModule'])) return null;
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

function safeDefault(installId: string): TrustScore {
  return {
    computedAt: new Date().toISOString(),
    installId,
    score: 100,
    signals: [],
  };
}

async function readL2(): Promise<null | CacheEntry> {
  const storage = loadAsyncStorage();
  if (!storage) return null;
  try {
    const raw = await storage.getItem(STORAGE_KEY);
    if (!raw) return null;
    const parsed = JSON.parse(raw) as CacheEntry;
    if (
      typeof parsed?.fetchedAt !== 'number' ||
      typeof parsed?.score?.score !== 'number'
    ) {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

async function writeL2(entry: CacheEntry): Promise<void> {
  const storage = loadAsyncStorage();
  if (!storage) return;
  try {
    await storage.setItem(STORAGE_KEY, JSON.stringify(entry));
  } catch {
    // best-effort
  }
}

async function fetchFresh(installId: string): Promise<TrustScore> {
  const config = getConfig();
  if (!config) return safeDefault(installId);
  try {
    const resp = await fetch(
      `${config.ingestUrl}/v1/security/score?installId=${encodeURIComponent(installId)}`,
      {
        headers: {
          Authorization: `Bearer ${config.token}`,
          'Sentori-Sdk': `react-native/${SDK_VERSION}`,
        },
        method: 'GET',
      }
    );
    if (!resp.ok) return safeDefault(installId);
    const parsed = (await resp.json()) as TrustScore;
    return parsed;
  } catch {
    return safeDefault(installId);
  }
}

/**
 * Resolve the device's trust score. Always resolves — never throws,
 * never rejects. The two-layer cache means the first call after cold
 * start usually serves a < 1 ms result while a background refresh
 * keeps the value current.
 */
export async function queryTrustScore(): Promise<TrustScore> {
  if (!isInitialized()) {
    const id = await getInstallId();
    return safeDefault(id);
  }
  // L1 hit
  const now = Date.now();
  if (_l1 && now - _l1.fetchedAt < L1_TTL_MS) {
    return _l1.score;
  }
  // Coalesce concurrent calls onto the same inflight fetch.
  if (_inflight) return _inflight;

  _inflight = (async () => {
    try {
      const installId = await getInstallId();
      // L2 hit — serve immediately, kick off background refresh
      const l2 = await readL2();
      if (l2 && now - l2.fetchedAt < L2_TTL_MS && l2.score.installId === installId) {
        _l1 = l2;
        // background refresh (don't block caller)
        void (async () => {
          const fresh = await fetchFresh(installId);
          const entry: CacheEntry = { fetchedAt: Date.now(), score: fresh };
          _l1 = entry;
          await writeL2(entry);
        })();
        return l2.score;
      }
      const fresh = await fetchFresh(installId);
      const entry: CacheEntry = { fetchedAt: Date.now(), score: fresh };
      _l1 = entry;
      await writeL2(entry);
      return fresh;
    } finally {
      _inflight = null;
    }
  })();
  return _inflight;
}

export function __resetTrustScoreForTests(): void {
  _l1 = null;
  _inflight = null;
}
