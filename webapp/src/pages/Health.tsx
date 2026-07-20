// Ops dashboard. Vitals + worker status + reference links.
// Polls /healthz every 10s.

import { useEffect, useState } from 'react';

import { api, HealthResponse } from '../lib/api';
import {
  Badge,
  Card,
  CardHeader,
  ErrorBanner,
  PageHeader,
  Section,
} from '../components/ui';

export function HealthPage() {
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [stamp, setStamp] = useState(new Date().toLocaleTimeString());

  function refresh() {
    setStamp(new Date().toLocaleTimeString());
    api
      .health()
      .then(setHealth)
      .catch((e: unknown) => setError(String(e)));
  }

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 10_000);
    return () => clearInterval(id);
  }, []);

  const poolPct =
    health?.pool_size && health.pool_idle != null
      ? Math.round((1 - health.pool_idle / health.pool_size) * 100)
      : null;

  return (
    <div className="space-y-4 p-6">
      <PageHeader
        title="Health"
        subtitle={`Live server vitals. Last refresh: ${stamp}.`}
        actions={
          <button
            onClick={refresh}
            className="rounded border border-zinc-700 px-3 py-1.5 text-sm hover:bg-zinc-800"
          >
            Refresh
          </button>
        }
      />

      {error && <ErrorBanner>{error}</ErrorBanner>}

      {health && (
        <>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
            <StatCard
              label="Status"
              value={
                <Badge tone={health.status === 'ok' ? 'ok' : 'neutral'}>
                  {health.status}
                </Badge>
              }
              sub={`v${health.version}`}
            />
            <StatCard
              label="Database"
              value={
                <Badge tone={health.db === 'ok' ? 'ok' : 'neutral'}>
                  {health.db}
                </Badge>
              }
              sub={
                health.pool_size != null && health.pool_idle != null
                  ? `${health.pool_size - health.pool_idle}/${health.pool_size} in use (${poolPct}%)`
                  : undefined
              }
            />
            <StatCard
              label="Push queued"
              value={
                <span className="font-mono text-xl text-zinc-100">
                  {health.push_queued ?? 0}
                </span>
              }
              sub="drained every 5s"
            />
            <StatCard
              label="Push failed 24h"
              value={
                <span
                  className={`font-mono text-xl ${
                    (health.push_failed_24h ?? 0) > 0
                      ? 'text-amber-400'
                      : 'text-zinc-100'
                  }`}
                >
                  {health.push_failed_24h ?? 0}
                </span>
              }
              sub="see /push-sends?status=failed"
            />
          </div>

          <Card>
            <CardHeader title="Scrape endpoints" />
            <Section>
              <ul className="space-y-1 text-xs font-mono text-zinc-400">
                <li>
                  GET <code>/healthz</code> — JSON snapshot (this page
                  uses it)
                </li>
                <li>
                  GET <code>/livez</code> — k8s livenessProbe (always
                  200; never restart on DB outage)
                </li>
                <li>
                  GET <code>/readyz</code> — k8s readinessProbe (200 if
                  DB up; 503 to shift traffic away)
                </li>
                <li>
                  GET <code>/metrics</code> — Prometheus text format
                  exposition (pool / push / events / issues / alerts /
                  sessions gauges)
                </li>
                <li>
                  GET <code>/v1/_describe</code> — endpoint catalog
                </li>
              </ul>
            </Section>
          </Card>

          <Card>
            <CardHeader title="Background workers" />
            <Section>
              <ul className="space-y-1 text-xs font-mono text-zinc-400">
                <li>
                  <span className="text-emerald-400">●</span>{' '}
                  push_worker — drains push_sends every{' '}
                  <code>SENTORI_PUSH_WORKER_INTERVAL_SEC</code> (5s)
                </li>
                <li>
                  <span className="text-emerald-400">●</span>{' '}
                  probe_worker — polls endpoint_check every{' '}
                  <code>SENTORI_PROBE_POLL_INTERVAL_SEC</code> (10s)
                </li>
                <li>
                  <span className="text-emerald-400">●</span>{' '}
                  archive_worker — DELETEs sent &gt;30d / failed &gt;90d
                  every <code>SENTORI_ARCHIVE_INTERVAL_SEC</code> (24h)
                </li>
                <li>
                  <span className="text-emerald-400">●</span>{' '}
                  periodic_alert_worker — evaluates crash_free_drop
                  rules every <code>SENTORI_PERIODIC_ALERT_INTERVAL_SEC</code>{' '}
                  (5min)
                </li>
                <li>
                  <span className="text-emerald-400">●</span> session
                  middleware — HttpOnly cookie + Bearer dual auth
                </li>
              </ul>
            </Section>
          </Card>

          <Card>
            <CardHeader title="Vendor adapters (push)" />
            <Section>
              <ul className="space-y-1 text-xs font-mono text-zinc-400">
                <li>
                  <span className="text-emerald-400">●</span> webpush —
                  VAPID ES256 wake push
                </li>
                <li>
                  <span className="text-emerald-400">●</span> apns —
                  token-based ES256 / HTTP/2 (cached JWT 55min)
                </li>
                <li>
                  <span className="text-emerald-400">●</span> fcm — legacy
                  HTTP API + body NotRegistered parsing
                </li>
                <li>
                  <span className="text-emerald-400">●</span> hcm — OAuth2
                  + HTTP push (cached bearer 55min)
                </li>
                <li>
                  <span className="text-emerald-400">●</span> mipush —
                  app-secret + form-encoded POST
                </li>
              </ul>
            </Section>
          </Card>

          <Card>
            <CardHeader title="Raw" />
            <Section>
              <pre className="overflow-auto rounded border border-zinc-800 bg-zinc-950 p-3 text-[10px] font-mono text-zinc-400">
                {JSON.stringify(health, null, 2)}
              </pre>
            </Section>
          </Card>
        </>
      )}
    </div>
  );
}

function StatCard({
  label,
  value,
  sub,
}: {
  label: string;
  value: React.ReactNode;
  sub?: string;
}) {
  return (
    <Card>
      <Section>
        <div className="text-xs uppercase tracking-wide text-zinc-500">
          {label}
        </div>
        <div className="mt-1">{value}</div>
        {sub && (
          <div className="mt-1 text-[10px] font-mono text-zinc-500">
            {sub}
          </div>
        )}
      </Section>
    </Card>
  );
}
