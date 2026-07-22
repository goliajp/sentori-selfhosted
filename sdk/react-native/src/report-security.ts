// v1.1 chunk S2 — security event reporting.
//
// `sentori.reportSecurity(kind, data)` POSTs a single security event
// to `/v1/security:report`. Helpers wrap common kinds with the right
// payload shape so dashboards can rely on it without coordinating
// schemas with every host app.
//
// Why a separate API from `captureException` / `track`: security
// reports have different retention + access patterns (the trust
// scoring engine in S3 reads them on a hot path) and conflating
// them with errors would pollute issue grouping. Single endpoint,
// no batching: pin mismatches and root-detection signals are
// low-volume by nature (one per app-lifetime in most cases).

import { getConfig, isInitialized } from './config';
import { peekInstallId } from './install-id';
import { getCurrentUserId } from './capture';

const SDK_VERSION = '0.0.0';

export type SecurityReportData = Record<string, unknown>;

/**
 * Report an arbitrary security signal. Fire-and-forget; resolves
 * with the server-assigned event id on success or `null` on any
 * failure (network down, server unhappy, SDK not initialised).
 *
 * Use the dedicated helpers (`reportPinMismatch`, future
 * `reportRootDetected`, …) when their shape applies — the dashboard
 * renders kind-specific panels off the well-known kinds.
 */
export async function reportSecurity(
  kind: string,
  data: SecurityReportData = {},
): Promise<null | string> {
  if (!isInitialized()) return null;
  if (typeof kind !== 'string' || kind.length === 0 || kind.length > 100) {
    return null;
  }
  if (Object.keys(data).length > 40) return null;

  const config = getConfig();
  if (!config) return null;
  const body = {
    kind,
    data,
    ts: new Date().toISOString(),
    userId: getCurrentUserId(),
    installId: peekInstallId() ?? undefined,
    release: config.release,
    environment: config.environment,
  };
  try {
    const resp = await fetch(`${config.ingestUrl}/v1/security:report`, {
      body: JSON.stringify(body),
      headers: {
        Authorization: `Bearer ${config.token}`,
        'Content-Type': 'application/json',
        'Sentori-Sdk': `react-native/${SDK_VERSION}`,
      },
      method: 'POST',
    });
    if (!resp.ok) return null;
    const parsed = (await resp.json().catch(() => null)) as null | { id?: string };
    return parsed?.id ?? null;
  } catch {
    return null;
  }
}

/**
 * v1.1 chunk S4 — link the current device's user / install to a
 * federated identity (e.g. a Google sub or Apple `sub`). Idempotent;
 * call on every sign-in. Posts to `/v1/security/link`. The dashboard
 * uses this to stitch the same user across projects in the Posture
 * cross-project view.
 *
 * Privacy: only the opaque OAuth `subject` value travels. Never pass
 * the email, display name, avatar, or any other identity attribute.
 */
export async function linkFederatedIdentity(args: {
  provider: string;
  subject: string;
  userId?: string;
}): Promise<boolean> {
  if (!args || typeof args.provider !== 'string' || args.provider.length === 0) return false;
  if (typeof args.subject !== 'string' || args.subject.length === 0) return false;
  if (!isInitialized()) return false;
  const config = getConfig();
  if (!config) return false;
  const installId = peekInstallId() ?? undefined;
  const body = {
    provider: args.provider,
    subject: args.subject,
    userId: args.userId ?? getCurrentUserId(),
    installId,
  };
  try {
    const resp = await fetch(`${config.ingestUrl}/v1/security/link`, {
      body: JSON.stringify(body),
      headers: {
        Authorization: `Bearer ${config.token}`,
        'Content-Type': 'application/json',
        'Sentori-Sdk': `react-native/${SDK_VERSION}`,
      },
      method: 'POST',
    });
    return resp.ok;
  } catch {
    return false;
  }
}

/**
 * TLS certificate pin mismatch — caller observed a server cert that
 * didn't match the configured pin set. Posts `kind = 'pin.mismatch'`
 * with the expected + observed pin (or hash) so the dashboard's
 * Pin anomaly panel can cluster reports by server.
 */
export async function reportPinMismatch(args: {
  expected: string;
  observed: string;
  /** Hostname the SDK was connecting to. Used by the dashboard to
   *  cluster reports per server. */
  serverName: string;
}): Promise<null | string> {
  if (!args || typeof args.serverName !== 'string' || args.serverName.length === 0) {
    return null;
  }
  // The serverName rides on the top-level envelope (column-typed on
  // the server) so dashboard queries don't need to crack the data
  // JSONB. We still echo it inside `data` for self-contained payloads.
  if (!isInitialized()) return null;
  const config = getConfig();
  if (!config) return null;
  const body = {
    kind: 'pin.mismatch',
    serverName: args.serverName,
    ts: new Date().toISOString(),
    userId: getCurrentUserId(),
    installId: peekInstallId() ?? undefined,
    release: config.release,
    environment: config.environment,
    data: {
      expected: args.expected,
      observed: args.observed,
    },
  };
  try {
    const resp = await fetch(`${config.ingestUrl}/v1/security:report`, {
      body: JSON.stringify(body),
      headers: {
        Authorization: `Bearer ${config.token}`,
        'Content-Type': 'application/json',
        'Sentori-Sdk': `react-native/${SDK_VERSION}`,
      },
      method: 'POST',
    });
    if (!resp.ok) return null;
    const parsed = (await resp.json().catch(() => null)) as null | { id?: string };
    return parsed?.id ?? null;
  } catch {
    return null;
  }
}
