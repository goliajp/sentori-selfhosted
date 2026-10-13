import { useReleaseNames } from '../lib/useReleaseNames';
import { ErrorBanner } from './ui';
import { Button } from '../../shared/design/controls';
import { platformLabel } from '../lib/platform-label';
import { useT } from '../i18n';
import { api } from '../lib/api';
import { formatCrashFree, trendDomain, type TrendPoint } from '../lib/crash-free';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';
function CrashFreeTrend({ points, hours }: { points: TrendPoint[]; hours: number }) {
  const t = useT();
  const W = 100;
  const H = 28;
  if (points.length < 2) return null;
  const [lo, hi] = trendDomain(points);
  const x = (i: number) => (i / (points.length - 1)) * W;
  const y = (v: number) => H - ((v - lo) / (hi - lo)) * H;

  // Each run of consecutive readings is its own path.
  const runs: string[] = [];
  let run: string[] = [];
  points.forEach((p, i) => {
    if (typeof p.crashFreeSessions !== 'number') {
      if (run.length > 1) runs.push(run.join(' '));
      run = [];
      return;
    }
    run.push(`${run.length === 0 ? 'M' : 'L'}${x(i).toFixed(2)},${y(p.crashFreeSessions).toFixed(2)}`);
  });
  if (run.length > 1) runs.push(run.join(' '));
  // A window of one reading surrounded by gaps has no line to draw;
  // say nothing rather than draw an empty box.
  if (runs.length === 0) return null;

  const worst = points.reduce<null | TrendPoint>(
    (acc, p) =>
      typeof p.crashFreeSessions === 'number' &&
      (acc === null || p.crashFreeSessions < (acc.crashFreeSessions ?? 100))
        ? p
        : acc,
    null,
  );

  return (
    <div className="mt-3" title={t('crashFree.trendTitle', { hours: String(hours), low: String(lo) })}>
      <svg
        viewBox={`0 0 ${W} ${H}`}
        preserveAspectRatio="none"
        className="h-8 w-full text-fg-muted"
        role="img"
        aria-label={t('crashFree.trendTitle', { hours: String(hours), low: String(lo) })}
      >
        {/* Not the error hue. The line is mostly the app working, and
            painting all of it red states a verdict the data does not —
            the dip shows because it is a dip. */}
        {runs.map((d) => (
          <path
            key={d}
            d={d}
            fill="none"
            stroke="currentColor"
            strokeWidth="1.5"
            vectorEffect="non-scaling-stroke"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        ))}
      </svg>
      <div className="mt-1 flex justify-between text-[10px] tabular-nums text-fg-subtle">
        <span>{t('crashFree.trendFrom', { hours: String(hours) })}</span>
        {worst && typeof worst.crashFreeSessions === 'number' && (
          <span>
            {t('crashFree.trendLow', {
              pct: formatCrashFree(worst.crashFreeSessions, worst.sessions, worst.crashedSessions),
            })}
          </span>
        )}
      </div>
    </div>
  );
}

export function CrashFreeCard({ projectId, appId, environment }: { projectId: string; appId?: string; environment?: string }) {
  const t = useT();
  const { data, error, reload } = useScopedAsyncData(JSON.stringify([projectId,appId,environment]), () => api.crashFree({ projectId, appId, environment, hours: 24 }));
  const releaseNames=useReleaseNames(projectId,(data?.releases??[]).map(row=>row.release));
  if (error) return <div className="surface"><ErrorBanner reason={error}>{t('inbox.loadFailed')}</ErrorBanner><Button onClick={reload}>{t('common.retry')}</Button></div>;
  if (!data) return null;

  // `typeof`, not `=== null`. This is a trust boundary: the number
  // arrives over HTTP, and a response without the field made
  // `pct.toFixed` throw and took the whole page white. The real
  // server answers a number or null; anything in between must render
  // as unknown rather than as a crash.
  const pct = typeof data.crashFreeSessions === 'number' ? data.crashFreeSessions : null;
  const releases = Array.isArray(data.releases) ? data.releases : [];
  // One crash in this many sessions moves the second decimal, so below
  // it the second decimal is noise.
  // Only the warning line. How many digits to print is
  // `formatCrashFree`'s job, and it needs the crash count too: cutting
  // precision must not cut the crash.
  const thin = (data.sessions ?? 0) < 1000;
  const hours = data.windowHours ?? 24;
  return (
    <section className="surface crash-free-card">
      <p className="text-xs subtle mb-3">{t('crashFree.scope')}</p>
      <div className="flex items-baseline justify-between">
        <h2 className="text-xs font-medium tracking-wide text-fg-muted uppercase">
          {t('crashFree.title')}
        </h2>
        <span className="text-xs text-fg-subtle">
          {t('crashFree.window', { hours: String(hours) })}
        </span>
      </div>

      {pct === null ? (
        <div className="crash-free-empty">
          <p className="text-sm text-fg-muted">{t('crashFree.empty')}</p>
          <p className="text-xs text-fg-subtle">{t('crashFree.emptyHint')}</p>
        </div>
      ) : (
        <>
          {/* Two decimals need a few hundred sessions to mean
              anything. Below that the digits are arithmetic, not
              measurement, and printing 90.00% off ten sessions claims
              a precision the data does not carry. */}
          <p className="mt-2 text-4xl font-semibold tabular-nums text-fg">
            {formatCrashFree(pct, data.sessions ?? 0, data.crashedSessions ?? 0)}
            <span className="ml-1 text-xl text-fg-muted">%</span>
          </p>
          {thin && <p className="mt-1 text-xs text-warn">{t('crashFree.thin')}</p>}
          {/* Said in words. A bare "1 / 10" makes the reader guess
              which number is which, and this dashboard has been
              caught doing that elsewhere. */}
          <p
            className="mt-1 text-xs text-fg-subtle tabular-nums"
            title={t('crashFree.sessionsTitle', { hours: String(hours) })}
          >
            {t('crashFree.counts', {
              crashed: String(data.crashedSessions ?? 0),
              total: String(data.sessions ?? 0),
            })}
          </p>
          {/* Sessions and users answer different questions. An app can
              be 99% crash-free by session and have hit a third of its
              users, because one person's ten quiet launches outvote
              their one crash. Both numbers are over the same window
              and the same user_key the inbox counts breadth by, so
              they are comparable — the title says which is which. */}
          <p
            className="mt-0.5 text-xs text-fg-subtle tabular-nums"
            title={t('crashFree.usersTitle', { hours: String(hours) })}
          >
            {typeof data.crashFreeUsers === 'number'
              ? t('crashFree.users', {
                  pct: formatCrashFree(
                    data.crashFreeUsers,
                    data.users ?? 0,
                    data.crashedUsers ?? 0,
                  ),
                })
              : t('crashFree.usersEmpty')}
          </p>
          <CrashFreeTrend points={Array.isArray(data.trend) ? data.trend : []} hours={hours} />
          {releases.length > 0 && (
            <ul className="mt-4 space-y-1 border-t border-border pt-3">
              {releases.slice(0, 5).map((r) => (
                <li
                  key={`${r.release}:${r.platform}`}
                  className="flex items-baseline gap-3 text-xs"
                >
                  <span className="min-w-0 flex-1 truncate font-mono text-fg-muted">
                    {releaseNames.label(r.release,{appId,platform:r.platform})}
                  </span>
                  <span className="shrink-0 text-fg-subtle">{platformLabel(r.platform, t)}</span>
                  <span className="w-16 shrink-0 text-right tabular-nums text-fg">
                    {typeof r.crashFreeSessions === 'number'
                      ? `${formatCrashFree(r.crashFreeSessions, r.sessions, r.crashedSessions)}%`
                      : '—'}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </>
      )}
    </section>
  );
}
