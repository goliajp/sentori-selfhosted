// Ops dashboard. Vitals + worker status + reference links.
// Polls /healthz every 10s.

import { useCallback, useEffect, useState } from 'react';

import { useT } from '../i18n';
import { api } from '../lib/api';
import { useAsyncData } from '../lib/useAsyncData';
import {
  Badge,
  Card,
  CardBody,
  CardHeader,
  ErrorBanner,
  PageHeader,
} from '../components/ui';

export function HealthPage() {
  const t = useT();
  const [stamp, setStamp] = useState(new Date().toLocaleTimeString());
  const { data: health, error, reload } = useAsyncData(
    () => api.health(),
    [],
    String,
  );

  const refresh = useCallback(() => {
    setStamp(new Date().toLocaleTimeString());
    reload();
  }, [reload]);

  useEffect(() => {
    const id = setInterval(refresh, 10_000);
    return () => clearInterval(id);
  }, [refresh]);

  const poolPct =
    health?.pool_size && health.pool_idle != null
      ? Math.round((1 - health.pool_idle / health.pool_size) * 100)
      : null;

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('health.title')}
        subtitle={t('health.subtitle').replace('{stamp}', stamp)}
        actions={
          <button
            onClick={refresh}
            className="inline-flex h-8 items-center rounded border border-border-strong px-3 text-sm hover:bg-raised"
          >{t('action.refresh')}</button>
        }
      />

      {error && <ErrorBanner>{error}</ErrorBanner>}

      {health && (
        <>
          <div className="grid grid-cols-2 gap-3 md:grid-cols-4">
            <StatCard
              label={t('crash.status')}
              value={
                <Badge tone={health.status === 'ok' ? 'ok' : 'neutral'}>
                  {health.status}
                </Badge>
              }
              sub={`v${health.version}`}
            />
            <StatCard
              label={t('health.database')}
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
              label={t('health.pushQueued')}
              value={
                <span className="font-mono text-xl text-fg">
                  {health.push_queued ?? 0}
                </span>
              }
              sub="drained every 5s"
            />
            <StatCard
              label={t('health.pushFailed')}
              value={
                <span
                  className={`font-mono text-xl ${
                    (health.push_failed_24h ?? 0) > 0
                      ? 'text-warn'
                      : 'text-fg'
                  }`}
                >
                  {health.push_failed_24h ?? 0}
                </span>
              }
              sub="see /push-sends?status=failed"
            />
          </div>

          <Card>
            <CardHeader title={t('health.scrape')} />
            <CardBody>
              <ul className="space-y-1 text-xs font-mono text-fg-muted">
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
            </CardBody>
          </Card>

          <Card>
            <CardHeader title={t('health.workers')} />
            <CardBody>
              <ul className="space-y-1 text-xs font-mono text-fg-muted">
                <li>
                  <span className="text-ok">●</span>{' '}
                  push_worker — drains push_sends every{' '}
                  <code>SENTORI_PUSH_WORKER_INTERVAL_SEC</code> (5s)
                </li>
                <li>
                  <span className="text-ok">●</span>{' '}
                  probe_worker — polls endpoint_check every{' '}
                  <code>SENTORI_PROBE_POLL_INTERVAL_SEC</code> (10s)
                </li>
                <li>
                  <span className="text-ok">●</span>{' '}
                  archive_worker — DELETEs sent &gt;30d / failed &gt;90d
                  every <code>SENTORI_ARCHIVE_INTERVAL_SEC</code> (24h)
                </li>
                <li>
                  <span className="text-ok">●</span>{' '}
                  periodic_alert_worker — evaluates crash_free_drop
                  rules every <code>SENTORI_PERIODIC_ALERT_INTERVAL_SEC</code>{' '}
                  (5min)
                </li>
                <li>
                  <span className="text-ok">●</span> session
                  middleware — HttpOnly cookie + Bearer dual auth
                </li>
              </ul>
            </CardBody>
          </Card>

          <Card>
            <CardHeader title={t('health.vendors')} />
            <CardBody>
              <ul className="space-y-1 text-xs font-mono text-fg-muted">
                <li>
                  <span className="text-ok">●</span> webpush —
                  VAPID ES256 wake push
                </li>
                <li>
                  <span className="text-ok">●</span> apns —
                  token-based ES256 / HTTP/2 (cached JWT 55min)
                </li>
                <li>
                  <span className="text-ok">●</span> fcm — legacy
                  HTTP API + body NotRegistered parsing
                </li>
                <li>
                  <span className="text-ok">●</span> hcm — OAuth2
                  + HTTP push (cached bearer 55min)
                </li>
                <li>
                  <span className="text-ok">●</span> mipush —
                  app-secret + form-encoded POST
                </li>
              </ul>
            </CardBody>
          </Card>

          <Card>
            <CardHeader title={t('health.raw')} />
            <CardBody>
              <pre className="overflow-auto rounded border border-border bg-bg p-3 text-xs font-mono text-fg-muted">
                {JSON.stringify(health, null, 2)}
              </pre>
            </CardBody>
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
      <CardBody>
        <div className="text-xs uppercase tracking-wide text-fg-subtle">
          {label}
        </div>
        <div className="mt-1">{value}</div>
        {sub && (
          <div className="mt-1 text-xs font-mono text-fg-subtle">
            {sub}
          </div>
        )}
      </CardBody>
    </Card>
  );
}
