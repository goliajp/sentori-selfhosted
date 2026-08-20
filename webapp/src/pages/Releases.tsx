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
import { lightColour, lightState } from '../lib/release-lights';

export default function ReleasesPage() {
  const t = useT();
  const { activeProject } = useShell();
  const active = activeProject?.id ?? null;

  const { data, error, loading, reload } = useAsyncData(
    () => (active ? api.listReleases(active) : Promise.resolve({ releases: [] })),
    [active],
  );

  return (
    <PageShell title={t('nav.releases')}>
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

function ReleaseRowView({
  release,
  projectId,
}: {
  release: ReleaseRow;
  projectId: string;
}) {
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
  // An artifact the server could not parse is not coverage. Counting
  // it lit is how a Hermes bytecode bundle passed as a source map on
  // two releases for months with a green light above it.
  const kinds = new Set(
    artifacts.filter((a) => a.usable !== false).map((a) => a.kind),
  );
  const unreadable = artifacts.filter((a) => a.usable === false);
  // A kind can hold a good artifact and a broken one at once — which
  // is exactly the shape that hid: the good map lights the row green
  // and the bundle somebody uploaded beside it says nothing until the
  // row is expanded. The light is the whole point of this screen, so
  // it carries both facts.
  const broken = new Set(unreadable.map((a) => a.kind));
  const used = new Set(release.platforms ?? []);
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
        {/* A light is only a warning for a platform this release
            actually hears from; the rest stay quiet furniture. */}
        <Light
          on={data ? kinds.has('sourcemap') : undefined}
          warn={broken.has('sourcemap')}
          label="js" used />
        <Light
          on={data ? kinds.has('dsym') : undefined}
          warn={broken.has('dsym')}
          label="ios"
          used={used.has('ios')}
        />
        <Light
          on={data ? kinds.has('proguard') : undefined}
          warn={broken.has('proguard')}
          label="android"
          used={used.has('android')}
        />
        {/* Source bundles are an enhancement — they put the failing
            line next to a native frame — so an absent one is never
            red. It is still worth showing: it was uploaded by the
            same pipeline step as the dSYM, and when that step stops
            being called this is the other half of what goes with
            it. */}
        <Light
          on={data ? kinds.has('srcbundle') : undefined}
          warn={broken.has('srcbundle')}
          label="src" />
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
                sentori-cli upload sourcemap --release &quot;{release.name}
                &quot; --token &lt;api-token&gt; &lt;map&gt;
              </code>
            </div>
          ) : (
            <div className="space-y-1">
              {unreadable.length > 0 && (
                <p className="mb-1.5 text-xs text-kind-error">
                  {t('releases.unreadable', {
                    names: unreadable.map((a) => a.name).join(', '),
                  })}
                </p>
              )}
              {artifacts.map((a) => (
                <div key={a.id} className="flex gap-3 font-mono text-xs">
                  <span className="w-20 text-fg-subtle">
                    {a.kind}
                    {a.usable === false && ' ✕'}
                  </span>
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

/** One symbolication light. `used` says whether this platform
 *  actually reports in the release: an absent artifact is red only
 *  then, and otherwise dims to furniture — three lights that go red
 *  regardless are noise, and noise buries the one that matters. */
function Light({
  on,
  label,
  used = false,
  warn = false,
}: {
  on: boolean | undefined;
  label: string;
  used?: boolean;
  /** This kind holds at least one artifact the reader could not
   *  parse. Green would be a lie even when a good one sits beside
   *  it — somebody uploaded a file believing it did something. */
  warn?: boolean;
}) {
  const t = useT();
  // The decision lives in `lib/release-lights` so a gate can see it:
  // this dot is the only thing most people read on this page, and it
  // rendered green over a broken artifact for months.
  const state = lightState({ broken: warn, on, used });
  const colour = lightColour(state);
  return (
    <span
      className={`flex items-center gap-1.5 font-mono text-xs ${
        used ? 'text-fg-muted' : 'text-fg-subtle/60'
      }`}
      title={
        warn
          ? t('releases.artifactBroken', { platform: label })
          : on === false && used
            ? t('releases.artifactMissing', { platform: label })
            : on === false
              ? t('releases.artifactUnused', { platform: label })
              : undefined
      }
    >
      <span className="h-2 w-2 rounded-full" style={{ backgroundColor: colour }} />
      {label}
    </span>
  );
}
