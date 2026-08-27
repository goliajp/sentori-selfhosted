// Projects — the layer above everything else. One card per
// project, carrying the pulse its SDK traffic reports (heartbeat,
// day counts, users, replay coverage, artifact lights). Picking a
// card scopes the whole app to that project and lands in its inbox
// — the Jira space model: the sidebar names where you are; this
// page is where you choose.

import { useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { useShell } from '../App';
import {
  Button,
  ErrorBanner,
  Input,
  PageShell,
  clsx,
  formatRelative,
  formatRelease,
} from '../components/ui';
import { useT } from '../i18n';
import { api, type Project, type ProjectHealth } from '../lib/api';
import { formatApiError, useAsyncData } from '../lib/useAsyncData';

type Row = Project & { health: ProjectHealth | null };

export default function ProjectsPage() {
  const t = useT();
  const { me, projects, activeProject, setActiveProjectId, reloadProjects } =
    useShell();
  const navigate = useNavigate();
  const owner = me.role === 'superadmin';
  const [name, setName] = useState('');
  const [createError, setCreateError] = useState<null | string>(null);

  // Captured per render, not called in render (react-hooks/purity).
  const [now] = useState(() => Date.now());
  const { data, loading } = useAsyncData<Row[]>(
    () =>
      Promise.all(
        projects.map(async (p) => ({
          ...p,
          health: await api.projectHealth(p.id).catch(() => null),
        })),
      ),
    [projects],
  );
  const rows = data ?? projects.map((p) => ({ ...p, health: null }));

  const open = (row: Row) => {
    setActiveProjectId(row.id);
    navigate('/');
  };

  return (
    <PageShell
      title={t('nav.projects')}
      toolbar={
        owner ? (
          <div className="ml-auto flex items-center gap-2">
            <Input
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={t('settings.projectName')}
              className="h-7 w-56 text-sm"
            />
            <Button
              size="sm"
              variant="primary"
              disabled={name.trim().length === 0}
              onClick={() => {
                setCreateError(null);
                void api.createProject(name.trim()).then(
                  () => {
                    setName('');
                    reloadProjects();
                  },
                  // Without this a rejected create — a duplicate name,
                  // a 500 — left the button looking unpressed.
                  (e: unknown) => setCreateError(formatApiError(e)),
                );
              }}
            >
              {t('settings.createProject')}
            </Button>
          </div>
        ) : undefined
      }
    >
      {createError && (
        <div className="mb-4">
          <ErrorBanner>
            {t('settings.createProjectFailed')} {createError}
          </ErrorBanner>
        </div>
      )}
      {/* An instance with no projects is where every operator starts,
          and the grid rendered nothing at all there — a blank page with
          a form in the corner, on the one screen that has to explain
          itself. */}
      {rows.length === 0 && !loading && (
        <div className="px-4 py-16 text-center">
          <p className="text-sm text-fg-muted">{t('projects.emptyTitle')}</p>
          <p className="mt-1.5 text-xs text-fg-subtle">
            {t('projects.emptyHint')}
          </p>
        </div>
      )}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-3">
        {rows.map((r) => (
          <ProjectCard
            key={r.id}
            row={r}
            now={now}
            active={r.id === activeProject?.id}
            owner={owner}
            onOpen={() => open(r)}
            onDelete={() => {
              if (window.confirm(t('settings.deleteProjectConfirm', { name: r.name }))) {
                void api.deleteProject(r.id).then(reloadProjects);
              }
            }}
          />
        ))}
      </div>
    </PageShell>
  );
}

function ProjectCard({
  row,
  now,
  active,
  owner,
  onOpen,
  onDelete,
}: {
  row: Row;
  now: number;
  active: boolean;
  owner: boolean;
  onOpen: () => void;
  onDelete: () => void;
}) {
  const t = useT();
  const h = row.health;
  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(e) => {
        if (e.key === 'Enter') onOpen();
      }}
      className={clsx(
        // Full-height flex column so neighbouring cards' footers sit
        // on one baseline whether or not a card has a backend row.
        'group flex h-full cursor-pointer flex-col rounded-lg border bg-surface p-4 transition-colors',
        active
          ? 'border-accent'
          : 'border-border hover:border-border-strong',
      )}
    >
      <div className="flex items-center gap-2">
        <Pulse health={h} now={now} />
        <span className="min-w-0 flex-1 truncate text-[15px] font-semibold text-fg">
          {row.name}
        </span>
        <span className="rounded bg-raised px-1.5 py-0.5 font-mono text-xs text-fg-muted">
          {row.platform}
        </span>
        {owner && (
          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              onDelete();
            }}
            className="text-xs text-kind-error/0 transition-colors group-hover:text-kind-error/70 hover:!text-kind-error"
          >
            {t('common.delete')}
          </button>
        )}
      </div>

      <div className="mt-3 flex items-baseline gap-5 text-sm tabular-nums">
        <span>
          <Num n={h?.counts24h.error} tone="error" />{' '}
          <span className="text-fg-subtle">err</span>
        </span>
        <span>
          <Num n={h?.counts24h.warn} tone="warn" />{' '}
          <span className="text-fg-subtle">warn</span>
        </span>
        <span>
          {/* `u` beside `err` and `warn` read as a truncated word rather
              than an abbreviation — the card has the width for it. */}
          <Num n={h?.users24h} /> <span className="text-fg-subtle">users</span>
        </span>
        {h && h.replay24h.eligible > 0 && (
          <span className="ml-auto text-fg-muted">
            {t('health.replay')} {h.replay24h.withScreens}/{h.replay24h.eligible}
          </span>
        )}
      </div>

      {h?.backend && (
        <div
          className="mt-2.5 flex items-center gap-2 text-sm tabular-nums"
          title={h.backend.url}
        >
          <span
            aria-hidden
            className="h-2 w-2 shrink-0 rounded-full"
            style={{
              backgroundColor:
                h.backend.lastOk === null
                  ? 'var(--sn-fg-muted)'
                  : h.backend.lastOk
                    ? 'var(--s-kind-probe)'
                    : 'var(--s-kind-error)',
            }}
          />
          <span className="text-fg-subtle">{t('health.backend')}</span>
          {h.backend.checks24h > 0 && (
            <span className="text-fg-muted">
              {((h.backend.ok24h / h.backend.checks24h) * 100).toFixed(1)}%
            </span>
          )}
          {h.backend.lastLatencyMs !== null && (
            <span className="ml-auto text-fg-subtle">{h.backend.lastLatencyMs}ms</span>
          )}
        </div>
      )}

      {/* the growing spacer pins the footer to the card's bottom,
          with a guaranteed minimum gap when content fills the card */}
      <div className="min-h-2.5 flex-1" />
      <div className="flex items-center gap-2 border-t border-border/60 pt-2.5 text-xs text-fg-subtle">
        {h?.latestRelease ? (
          <>
            <span className="min-w-0 flex-1 truncate font-mono" title={h.latestRelease}>
              {formatRelease(h.latestRelease)}
            </span>
            <ArtifactLights
              kinds={h.latestReleaseArtifacts}
              used={h.platforms24h}
            />
          </>
        ) : (
          <span className="flex-1">—</span>
        )}
        <span className="shrink-0">
          {h?.lastEventAt ? formatRelative(h.lastEventAt) : t('health.silent')}
        </span>
      </div>
    </div>
  );
}

function Pulse({ health, now }: { health: ProjectHealth | null; now: number }) {
  const last = health?.lastEventAt ? Date.parse(health.lastEventAt) : null;
  const age = last === null ? null : now - last;
  const color =
    age === null
      ? 'var(--sn-fg-muted)'
      : age < 10 * 60_000
        ? 'var(--s-kind-probe)'
        : age < 60 * 60_000
          ? 'var(--s-kind-warn)'
          : 'var(--s-kind-error)';
  return (
    <span
      aria-hidden
      className="h-2 w-2 shrink-0 rounded-full"
      style={{ backgroundColor: color }}
    />
  );
}

function Num({ n, tone }: { n: number | undefined; tone?: 'error' | 'warn' }) {
  if (n === undefined || n === 0)
    return <span className="text-fg-subtle">{n ?? '—'}</span>;
  return (
    <span
      className="text-xs font-medium tabular-nums"
      style={tone ? { color: `var(--s-kind-${tone})` } : undefined}
    >
      {n}
    </span>
  );
}

/** The three symbolication lights, same rule as the Releases page:
 *  a missing artifact is only a problem for a platform this project
 *  actually hears from. A pure-iOS app with a permanently red
 *  proguard light teaches the reader to ignore all three. */
function ArtifactLights({
  kinds,
  used,
}: {
  kinds: string[];
  used: Record<string, number>;
}) {
  const t = useT();
  // JS runs on every platform that reports at all, so a sourcemap is
  // owed whenever anything is coming in.
  const anyTraffic = Object.values(used).some((n) => n > 0);
  const lights: [string, string, boolean][] = [
    ['js', 'sourcemap', anyTraffic],
    ['ios', 'dsym', (used.ios ?? 0) > 0],
    ['android', 'proguard', (used.android ?? 0) > 0],
  ];
  return (
    <span className="flex shrink-0 gap-1.5 text-xs">
      {lights.map(([label, kind, live]) => {
        const have = kinds.includes(kind);
        return (
          <span
            key={label}
            title={
              have
                ? undefined
                : live
                  ? t('releases.artifactMissing', { platform: label })
                  : t('releases.artifactUnused', { platform: label })
            }
            style={{
              color: have
                ? 'var(--s-kind-probe)'
                : live
                  ? 'var(--s-kind-error)'
                  : 'var(--sn-fg-subtle)',
            }}
          >
            {label}
          </span>
        );
      })}
    </span>
  );
}
