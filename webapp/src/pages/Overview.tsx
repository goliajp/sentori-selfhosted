import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';
import { useT } from '../i18n';
import { api, ApiError, Project, ProjectStats, UsageResponse } from '../lib/api';
import { Sparkline } from '../components/Sparkline';
import {
  Card,
  CardHeader,
  ErrorBanner,
  PageHeader,
  formatNumber,
} from '../components/ui';

export function OverviewPage() {
  const t = useT();
  const [projects, setProjects] = useState<Project[] | null>(null);
  const [usage, setUsage] = useState<UsageResponse | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [trends, setTrends] = useState<Record<string, number[]>>({});
  const [stats, setStats] = useState<Record<string, ProjectStats>>({});

  useEffect(() => {
    Promise.all([api.listProjects(), api.usage()])
      .then(async ([p, u]) => {
        setProjects(p);
        setUsage(u);
        // Fetch 7-day trend + 24h lens stats per project in parallel
        const [t, s] = await Promise.all([
          Promise.all(
            p.map(async pr => {
              try {
                const series = await api.eventsTrend(pr.id, 7);
                return [pr.id, series.map(d => d.count)] as const;
              } catch {
                return [pr.id, []] as const;
              }
            }),
          ),
          Promise.all(
            p.map(async pr => {
              try {
                const st = await api.projectStats(pr.id);
                return [pr.id, st] as const;
              } catch {
                return [pr.id, null] as const;
              }
            }),
          ),
        ]);
        setTrends(Object.fromEntries(t));
        setStats(
          Object.fromEntries(
            s.filter(([, v]) => v !== null) as [string, ProjectStats][],
          ),
        );
      })
      .catch((e: unknown) => {
        if (e instanceof ApiError) setErr(`${e.status}: ${e.body}`);
        else setErr(String(e));
      });
  }, []);

  return (
    <div>
      <PageHeader
        title={t('overview.title')}
        subtitle={t('overview.subtitle')}
      />
      {err && <ErrorBanner>{err}</ErrorBanner>}

      {usage && (
        <div className="mb-6 grid grid-cols-3 gap-4">
          <UsageCard title={t('events.title')} {...usage.events} />
          <UsageCard title={t('overview.spans')} {...usage.spans} />
          <UsageCard title={t('replays.title')} {...usage.replays} />
        </div>
      )}

      <Card>
        <CardHeader
          title={t('overview.projects')}
          subtitle={
            projects
              ? projects.length === 1
                ? t('overview.projectCountOne')
                : t('overview.projectCount').replace('{n}', String(projects.length))
              : t('common.loading')
          }
        />
        {projects?.length === 0 ? (
          <OnboardingGuide />
        ) : (
          <ul className="divide-y divide-border">
            {projects?.map((p) => (
              <li
                key={p.id}
                className="flex items-center justify-between px-5 py-3"
              >
                <div className="min-w-0 flex-1">
                  <Link
                    to={`/projects/${p.id}/issues`}
                    className="text-sm font-medium text-fg hover:text-accent"
                  >
                    {p.name}
                  </Link>
                  <p className="font-mono text-xs text-fg-subtle">
                    {p.slug}
                  </p>
                </div>
                <div className="flex items-center gap-4">
                  {stats[p.id] && (
                    <div className="flex gap-2 text-xs">
                      <LensPill
                        label={t('events.title')}
                        value={stats[p.id].events_24h}
                      />
                      <LensPill
                        label={t('overview.activeIssues')}
                        value={stats[p.id].issues_active}
                        tone="warn"
                      />
                      <LensPill
                        label={t('overview.spans')}
                        value={stats[p.id].spans_24h}
                      />
                      <LensPill
                        label={t('overview.metrics')}
                        value={stats[p.id].metrics_buckets_24h}
                      />
                      <LensPill
                        label={t('replays.title')}
                        value={stats[p.id].replays_24h}
                      />
                    </div>
                  )}
                  <Sparkline
                    values={trends[p.id] ?? []}
                    width={120}
                    height={32}
                  />
                  <Link
                    to={`/projects/${p.id}/issues`}
                    className="rounded bg-raised px-3 py-1 text-xs text-fg-muted hover:bg-raised"
                  >
                    {t('overview.viewIssues')}
                  </Link>
                </div>
              </li>
            ))}
          </ul>
        )}
      </Card>
    </div>
  );
}

function OnboardingGuide() {
  const t = useT();
  const steps = [
    {
      n: 1,
      title: t('overview.step1Title'),
      body: t('overview.step1Body'),
    },
    {
      n: 2,
      title: t('overview.step2Title'),
      body: t('overview.step2Body'),
    },
    {
      n: 3,
      title: t('overview.step3Title'),
      body: t('overview.step3Body'),
    },
    {
      n: 4,
      title: t('overview.step4Title'),
      body: t('overview.step4Body'),
    },
  ];
  return (
    <div>
      <div className="mx-auto max-w-2xl">
        <h3 className="text-base font-medium text-fg">{t('auth.welcome')}</h3>
        <p className="mt-1 text-sm text-fg-subtle">
          Four steps to your first event. Start by creating a project.
        </p>
        <ol className="mt-6 space-y-4">
          {steps.map(s => (
            <li key={s.n} className="flex gap-3">
              <span className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full bg-raised text-xs font-medium text-fg-muted">
                {s.n}
              </span>
              <div>
                <p className="text-sm font-medium text-fg">{s.title}</p>
                <p className="text-xs text-fg-subtle">{s.body}</p>
              </div>
            </li>
          ))}
        </ol>
        <div className="mt-6">
          <Link
            to="/projects"
            className="inline-flex rounded bg-accent px-4 py-2 text-sm font-medium text-accent-fg hover:bg-accent"
          >
            Create your first project →
          </Link>
        </div>
      </div>
    </div>
  );
}

function UsageCard({
  title,
  count,
  dropped,
  limit,
}: {
  title: string;
  count: number;
  dropped: number;
  limit: number;
}) {
  const t = useT();
  const pct = limit > 0 && limit < Number.MAX_SAFE_INTEGER
    ? Math.min(100, Math.round((count / limit) * 100))
    : 0;
  const isUnlimited = limit >= Number.MAX_SAFE_INTEGER || limit > 1e15;
  return (
    <div className="rounded border border-border bg-surface px-5 py-4">
      <p className="text-xs uppercase tracking-wide text-fg-subtle">{title}</p>
      <p className="mt-1 font-mono text-2xl text-fg">{formatNumber(count)}</p>
      <p className="text-xs text-fg-subtle">
        {isUnlimited
          ? t('overview.unlimited')
          : t('overview.ofPerMonth')
              .replace('{limit}', formatNumber(limit))
              .replace('{pct}', String(pct))}
      </p>
      {dropped > 0 && (
        <p className="mt-1 text-xs text-danger">
          dropped: {formatNumber(dropped)}
        </p>
      )}
      {!isUnlimited && (
        <div className="mt-2 h-1 overflow-hidden rounded bg-raised">
          <div
            className="h-full bg-accent transition-all"
            style={{ width: `${pct}%` }}
          />
        </div>
      )}
    </div>
  );
}

function LensPill({
  label,
  value,
  tone,
}: {
  label: string;
  value: number;
  tone?: 'warn';
}) {
  if (value === 0) {
    return (
      <span className="rounded bg-surface px-1.5 py-0.5 font-mono text-fg-subtle">
        {label} 0
      </span>
    );
  }
  return (
    <span
      className={`rounded px-1.5 py-0.5 font-mono ${
        tone === 'warn'
          ? 'bg-warn/40 text-warn'
          : 'bg-raised text-fg-muted'
      }`}
    >
      {label} {formatNumber(value)}
    </span>
  );
}
