// v0.9.0 #10 — EAS Update / CodePush awareness.
//
// At capture time we want to know which JS bundle the user is running:
// it's almost always an OTA update rather than the binary `release`,
// and crash spikes correlate to a specific bundle id rather than the
// app version. We try `expo-updates` first (EAS), then `react-native-
// code-push`, then nothing. All access is `require()`-shielded so the
// SDK still works when neither is installed.
//
// Cached at module load — bundle id doesn't change mid-session in
// either Expo or CodePush.

export type BundleInfo = {
  /** Stable identifier — Expo `updateId` or CodePush `label`. */
  id: string;
  /** When the bundle was published. RFC 3339. */
  deployedAt?: string;
  /** Which OTA system reported it. */
  source: 'codepush' | 'expo';
};

let _cached: BundleInfo | null | undefined;

export function getBundleInfo(): BundleInfo | null {
  if (_cached !== undefined) return _cached;
  _cached = detect();
  return _cached;
}

function detect(): BundleInfo | null {
  // Expo Updates first — most modern RN deployments are on EAS Update.
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const Updates = require('expo-updates') as {
      commitTime?: Date | null;
      isEmbeddedLaunch?: boolean;
      manifest?: { id?: string; createdAt?: string };
      updateId?: null | string;
    };
    const id = Updates.updateId ?? Updates.manifest?.id;
    if (typeof id === 'string' && id.length > 0) {
      const deployedAt = pickDeployedAt(Updates);
      return { deployedAt, id, source: 'expo' };
    }
  } catch {
    // expo-updates not installed
  }
  // CodePush fallback.
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const cp = require('react-native-code-push') as {
      getCurrentPackage?: () => Promise<{
        appVersion?: string;
        label?: string;
        packageHash?: string;
      } | null>;
    };
    // `getCurrentPackage` is async; we don't block init. Schedule a
    // background fetch + populate the cache. First-event-after-init
    // may miss the bundle id; subsequent events will have it.
    if (typeof cp.getCurrentPackage === 'function') {
      void cp
        .getCurrentPackage()
        .then((pkg) => {
          if (pkg && (pkg.label || pkg.packageHash)) {
            _cached = {
              id: pkg.label ?? pkg.packageHash!,
              source: 'codepush',
            };
          }
        })
        .catch(() => {
          // ignore
        });
    }
  } catch {
    // not installed
  }
  return null;
}

function pickDeployedAt(u: {
  commitTime?: Date | null;
  manifest?: { createdAt?: string };
}): string | undefined {
  if (u.commitTime instanceof Date) return u.commitTime.toISOString();
  const ts = u.manifest?.createdAt;
  if (typeof ts === 'string') return ts;
  return undefined;
}

/** Test-only — reset cache. */
export function __resetBundleInfoForTests(): void {
  _cached = undefined;
}
