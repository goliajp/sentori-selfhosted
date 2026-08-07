// Releases — "did this version ship healthy?" (design.md §11).
// One row per release: the three symbolication lights (sourcemap /
// dsym / proguard), backed by upload commands when a light is off.
// Artifact gaps are most visible here, on purpose — the lights are
// live on load, not hidden behind a click.

import { ChevronRight } from 'lucide-react';
import { useState } from 'react';

import { useShell } from '../App';
import {
  ErrorBanner,
  PageShell,
  Panel,
  PanelEmpty,
  formatRelative,
} from '../components/ui';
import { useT } from '../i18n';
import { api, type ReleaseRow } from '../lib/api';
import { useAsyncData } from '../lib/useAsyncData';

export default function ReleasesPage() {
  const t = useT();
  const { activeProject } = useShell();
  const active = activeProject?.id ?? null;

  const { data, error, loading, reload } = useAsyncData(
    () => (active ? api.listReleases(active) : Promise.resolve({ releases: [] })),
    [active],
  );

  return (
    <PageShell
      title={t('nav.releases')}
    >
      {error && (
        <ErrorBanner>
          {t('releases.loadFailed')}{' '}
          <button type="button" className="underline" onClick={reload}>
            {t('common.retry')}
          </button>
        </ErrorBanner>
      )}
      {loading && !data && (
        <div className="py-16 text-center text-sm text-fg-subtle">…</div>
      )}

      <Panel title={`${t('nav.releases')} (${data?.releases.length ?? 0})`}>
        {data && data.releases.length === 0 ? (
          <PanelEmpty>
            {t('releases.emptyTitle')} — {t('releases.emptyHint')}
          </PanelEmpty>
        ) : (
          <div className="divide-y divide-border/60">
            {(data?.releases ?? []).map((r) => (
              <ReleaseRowView key={r.id} release={r} projectId={active ?? ''} />
            ))}
          </div>
        )}
      </Panel>
    </PageShell>
  );
}

function ReleaseRowView({ release, projectId }: { release: ReleaseRow; projectId: string }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  // Artifacts load with the row: the lights ARE the page — greying
  // them out until a click would hide exactly the gap this screen
  // exists to show.
  const { data } = useAsyncData(
    () => api.listArtifacts(projectId, release.id),
    [release.id],
  );
  const artifacts = data?.artifacts ?? [];
  const kinds = new Set(artifacts.map((a) => a.kind));
  const created = release.createdAt ?? release.created_at;

  return (
    <div>
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
        className="flex w-full items-center gap-4 px-3.5 py-2 text-left hover:bg-raised/50"
      >
        <ChevronRight
          aria-hidden
          className={`h-3.5 w-3.5 shrink-0 text-fg-subtle transition-transform ${open ? 'rotate-90' : ''}`}
        />
        <span className="min-w-0 flex-1 truncate font-mono text-sm text-fg">
          {release.name}
        </span>
        <Light on={data ? kinds.has('sourcemap') : undefined} label="js" />
        <Light on={data ? kinds.has('dsym') : undefined} label="ios" />
        <Light on={data ? kinds.has('proguard') : undefined} label="android" />
        {created && (
          <span className="w-16 text-right text-xs tabular-nums text-fg-subtle">
            {formatRelative(created)}
          </span>
        )}
      </button>
      {open && (
        <div className="border-t border-border/60 bg-bg px-3.5 py-2.5 pl-10">
          {artifacts.length === 0 ? (
            <div className="text-xs text-fg-muted">
              <p>{t('releases.noArtifacts')}</p>
              <code className="mt-1.5 block rounded bg-surface p-2 font-mono text-xs">
                sentori-cli upload sourcemap --release &quot;{release.name}&quot; --token
                &lt;api-token&gt; &lt;map&gt;
              </code>
            </div>
          ) : (
            <div className="space-y-1">
              {artifacts.map((a) => (
                <div key={a.id} className="flex gap-3 font-mono text-xs">
                  <span className="w-20 text-fg-subtle">{a.kind}</span>
                  <span className="min-w-0 flex-1 truncate text-fg-muted">{a.name}</span>
                  {a.size_bytes !== undefined && (
                    <span className="text-fg-subtle">
                      {Math.round(a.size_bytes / 1024)} KB
                    </span>
                  )}
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  );
}

function Light({ on, label }: { on: boolean | undefined; label: string }) {
  return (
    <span className="flex items-center gap-1.5 font-mono text-xs text-fg-muted">
      <span
        className="h-2 w-2 rounded-full"
        style={{
          backgroundColor:
            on === undefined
              ? 'color-mix(in srgb, var(--sn-fg-muted) 30%, transparent)'
              : on
                ? 'var(--s-kind-probe)'
                : 'var(--s-kind-error)',
        }}
      />
      {label}
    </span>
  );
}
