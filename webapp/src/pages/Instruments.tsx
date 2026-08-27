// Instruments — "how are the devices I planted doing?" (design.md
// §11). Three tables: asserts (alive + failure rate, "ran 45k,
// failed 3"), probes (silent = fix holding), traces (did it run,
// what magnitude). A status surface, not a data browser — but a
// status surface with real columns, headers, and one baseline.

import { Link } from 'react-router-dom';

import { useShell } from '../App';
import {
  DataTable,
  ErrorBanner,
  PageShell,
  Panel,
  PanelEmpty,
  formatRelative,
  formatRelease,
} from '../components/ui';
import { useT } from '../i18n';
import { useAsyncData } from '../lib/useAsyncData';

type AssertRow = {
  name: string;
  release: string;
  passCount: number;
  failCount: number;
  lastPassAt: string | null;
  lastFailAt: string | null;
};
type ProbeRow = {
  ref: string;
  issueId: string | null;
  lastSeenRelease: string | null;
  registeredAt: string;
  lastFiredAt: string | null;
  fireCount: number;
};
type TraceRow = {
  name: string;
  eventCount: number;
  usersCount: number;
  lastSeen: string;
};
type LaunchRow = {
  release: string;
  samples: number;
  prewarmed: number;
  p50: null | number;
  p90: null | number;
  p95: null | number;
};
type Instruments = {
  asserts: AssertRow[];
  probes: ProbeRow[];
  traces: TraceRow[];
  launch?: LaunchRow[];
};

const fmtMs = (v: null | number) =>
  v === null ? '—' : v >= 10_000 ? `${(v / 1000).toFixed(1)}s` : `${Math.round(v)}ms`;

/** The green/red life sign in a table's first column. */
function Dot({ ok }: { ok: boolean }) {
  return (
    <span
      aria-hidden
      className="inline-block h-2 w-2 rounded-full align-middle"
      style={{
        backgroundColor: ok ? 'var(--s-kind-probe)' : 'var(--s-kind-error)',
      }}
    />
  );
}

function ReleaseCell({ release }: { release: string | null }) {
  if (!release) return <span>—</span>;
  return (
    <span className="font-mono text-xs text-fg-subtle" title={release}>
      {formatRelease(release)}
    </span>
  );
}

export default function InstrumentsPage() {
  const t = useT();
  const { activeProject } = useShell();
  const active = activeProject?.id ?? null;

  const { data, error, loading, reload } = useAsyncData<Instruments | null>(
    () => (active ? fetchInstruments(active) : Promise.resolve(null)),
    [active],
  );

  return (
    <PageShell
      title={t('nav.instruments')}
    >
      {error && (
        <ErrorBanner>
          {t('instruments.loadFailed')}{' '}
          <button type="button" className="underline" onClick={reload}>
            {t('common.retry')}
          </button>
        </ErrorBanner>
      )}
      {loading && !data && (
        <div className="py-16 text-center text-sm text-fg-subtle">…</div>
      )}
      {!active && <PanelEmpty>{t('instruments.noProject')}</PanelEmpty>}

      {data && (
        <>
          {/* Launch percentiles (QIP-8 #3): the regression gate the
              >3s warn alone can't be — median-vs-devices separable,
              prewarmed phantoms counted but excluded. */}
          <Panel title={`${t('instruments.launch')}${data.launch?.length ? ` (${data.launch.length})` : ''}`}>
            {!data.launch?.length ? (
              <PanelEmpty>{t('instruments.launchEmpty')}</PanelEmpty>
            ) : (
              <DataTable<LaunchRow>
                rows={data.launch}
                rowKey={(l) => l.release}
                columns={[
                  {
                    key: 'release',
                    label: t('instruments.colRelease'),
                    render: (l) => <ReleaseCell release={l.release} />,
                  },
                  // Two columns, not `4906 (+214)` in one. `(+N)` is
                  // how every other number on every other screen
                  // writes a change, so a reader has no reason to
                  // hover the tooltip that said otherwise — and read
                  // 1.2.2's `0 (+3)` as a release that gained three
                  // launches rather than one with no real samples and
                  // three phantom ones. A header cannot be misread
                  // the way a punctuation convention can.
                  {
                    key: 'samples',
                    label: t('instruments.colSamples'),
                    width: '110px',
                    align: 'right',
                    render: (l) => (
                      <span className="text-sm tabular-nums text-fg-muted">
                        {l.samples - l.prewarmed}
                      </span>
                    ),
                  },
                  {
                    key: 'prewarmed',
                    label: t('instruments.colPrewarmed'),
                    width: '110px',
                    align: 'right',
                    render: (l) => (
                      <span
                        className="text-sm tabular-nums text-fg-subtle"
                        title={t('instruments.prewarmedTip')}
                      >
                        {l.prewarmed > 0 ? l.prewarmed : '—'}
                      </span>
                    ),
                  },
                  ...(['p50', 'p90', 'p95'] as const).map((p) => ({
                    key: p,
                    label: p,
                    width: '100px',
                    align: 'right' as const,
                    render: (l: LaunchRow) => (
                      <span
                        className={`text-sm tabular-nums ${
                          (l[p] ?? 0) > 3000 ? 'text-kind-warn' : 'text-fg-muted'
                        }`}
                      >
                        {fmtMs(l[p])}
                      </span>
                    ),
                  })),
                ]}
              />
            )}
          </Panel>

          <Panel title={`${t('instruments.asserts')} (${data.asserts.length})`}>
            <DataTable<AssertRow>
              rows={data.asserts}
              rowKey={(a) => `${a.name}-${a.release}`}
              empty={t('instruments.assertsEmpty')}
              columns={[
                {
                  key: 'name',
                  label: t('instruments.colName'),
                  render: (a) => (
                    <span className="inline-flex items-center gap-2.5 font-mono text-sm text-fg">
                      <Dot ok={a.failCount === 0} />
                      {a.name}
                    </span>
                  ),
                },
                {
                  key: 'release',
                  label: t('instruments.colRelease'),
                  // Right-aligned and narrow so it sits next to the
                  // numbers rather than stranded in the middle: the
                  // name column absorbs every pixel of slack, and a
                  // left-aligned column after it starts wherever that
                  // slack ends — a river of white down the table.
                  width: '160px',
                  align: 'right',
                  render: (a) => <ReleaseCell release={a.release} />,
                },
                {
                  key: 'ran',
                  label: t('instruments.colStatus'),
                  width: '220px',
                  align: 'right',
                  render: (a) => (
                    <span className="text-sm tabular-nums text-fg-muted">
                      {t('instruments.assertRan', {
                        total: String(a.passCount + a.failCount),
                        failed: String(a.failCount),
                      })}
                    </span>
                  ),
                },
                {
                  key: 'lastFailAt',
                  label: t('instruments.colLastFail'),
                  width: '110px',
                  align: 'right',
                  render: (a) => (
                    <span
                      className={`text-xs tabular-nums ${
                        a.lastFailAt ? 'text-kind-error' : 'text-fg-subtle'
                      }`}
                    >
                      {a.lastFailAt ? formatRelative(a.lastFailAt) : '—'}
                    </span>
                  ),
                },
              ]}
            />
          </Panel>

          <Panel title={`${t('instruments.probes')} (${data.probes.length})`}>
            <DataTable<ProbeRow>
              rows={data.probes}
              rowKey={(p) => p.ref}
              empty={t('instruments.probesEmpty')}
              columns={[
                {
                  key: 'ref',
                  label: t('instruments.colName'),
                  render: (p) => (
                    <span className="inline-flex items-center gap-2.5 font-mono text-sm text-fg">
                      <Dot ok={p.fireCount === 0} />
                      {p.ref}
                    </span>
                  ),
                },
                {
                  key: 'lastSeenRelease',
                  label: t('instruments.colRelease'),
                  width: '160px',
                  align: 'right',
                  render: (p) => <ReleaseCell release={p.lastSeenRelease} />,
                },
                {
                  key: 'status',
                  label: t('instruments.colStatus'),
                  width: '300px',
                  align: 'right',
                  render: (p) => (
                    <span className="text-sm tabular-nums text-fg-muted">
                      {p.fireCount === 0
                        ? t('instruments.probeSilent', {
                            since: formatRelative(p.registeredAt),
                          })
                        : t('instruments.probeFired', {
                            count: String(p.fireCount),
                            last: p.lastFiredAt ? formatRelative(p.lastFiredAt) : '',
                          })}
                    </span>
                  ),
                },
                {
                  key: 'issueId',
                  label: '',
                  width: '110px',
                  align: 'right',
                  render: (p) =>
                    p.issueId ? (
                      <Link
                        to={`/issues/${p.issueId}`}
                        className="text-xs text-fg-muted underline hover:text-fg"
                      >
                        {t('instruments.guardedIssue')}
                      </Link>
                    ) : (
                      ''
                    ),
                },
              ]}
            />
          </Panel>

          <Panel title={`${t('instruments.traces')} (${data.traces.length})`}>
            <DataTable<TraceRow>
              rows={data.traces}
              rowKey={(tr) => tr.name}
              empty={t('instruments.tracesEmpty')}
              columns={[
                {
                  key: 'name',
                  label: t('instruments.colName'),
                  render: (tr) => (
                    <span className="font-mono text-sm text-fg">{tr.name}</span>
                  ),
                },
                {
                  key: 'volume',
                  label: t('instruments.colVolume'),
                  width: '180px',
                  align: 'right',
                  render: (tr) => (
                    <span className="text-sm tabular-nums text-fg-muted">
                      {/* Spelled out: this column is 180px wide, and `0u`
                          beside `2ev` reads as a truncated word. The
                          queue rows in kind.tsx keep the compact form —
                          there the space really is the constraint. */}
                      {t('instruments.volume', {
                        events: String(tr.eventCount),
                        users: String(tr.usersCount),
                      })}
                    </span>
                  ),
                },
                {
                  key: 'lastSeen',
                  label: t('instruments.colLastSeen'),
                  width: '110px',
                  align: 'right',
                  render: (tr) => (
                    <span className="text-xs tabular-nums text-fg-subtle">
                      {formatRelative(tr.lastSeen)}
                    </span>
                  ),
                },
              ]}
            />
          </Panel>
        </>
      )}
    </PageShell>
  );
}

async function fetchInstruments(projectId: string): Promise<Instruments> {
  const resp = await fetch(`/admin/api/projects/${projectId}/instruments`, {
    credentials: 'include',
  });
  if (!resp.ok) throw new Error(String(resp.status));
  return (await resp.json()) as Instruments;
}
