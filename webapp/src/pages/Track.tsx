// What the app told us people did.
//
// `track_events` had an ingest route since migration 0019 and 19k rows
// in production, with nothing able to read them back. This is the
// screen that opens the table.
//
// It is not a general analytics product and should not grow into one.
// The question it answers is the one next to a crash: this user, or
// this release, was doing *what* — the breadcrumb timeline widened
// from a single event to the whole stream.

import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';

import { useT } from '../i18n';
import { api, ApiError, TrackEventRow, TrackName } from '../lib/api';
import {
  Card,
  CardBody,
  CardHeader,
  EmptyState,
  ErrorBanner,
  PageHeader,
  Select,
  formatNumber,
  formatRelative,
} from '../components/ui';
import { useProjectName } from '../lib/useProjectName';

const WINDOWS = [1, 7, 30, 90] as const;

export default function Track() {
  const t = useT();
  const { id: projectId } = useParams<{ id: string }>();
  const projectName = useProjectName(projectId);
  const [days, setDays] = useState<number>(7);
  const [names, setNames] = useState<TrackName[] | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [recent, setRecent] = useState<TrackEventRow[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!projectId) return;
    api
      .trackNames(projectId, days)
      .then(r => setNames(r.names))
      .catch((e: unknown) => {
        if (e instanceof ApiError) setError(`${e.status}: ${e.body}`);
        else setError(String(e));
      });
  }, [projectId, days]);

  useEffect(() => {
    if (!projectId) return;
    api
      .trackRecent(projectId, { name: selected ?? undefined, limit: 50 })
      .then(r => setRecent(r.events))
      .catch(() => setRecent([]));
  }, [projectId, selected]);

  if (!projectId) return null;

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('track.title')}
        subtitle={projectName}
        actions={
          <Select
            value={String(days)}
            onChange={e => setDays(Number(e.target.value))}
          >
            {WINDOWS.map(d => (
              <option key={d} value={d}>
                {t('track.lastDays').replace('{n}', String(d))}
              </option>
            ))}
          </Select>
        }
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {names?.length === 0 ? (
        <EmptyState title={t('track.empty')} hint={t('track.emptyHint')} />
      ) : (
        <Card>
          <CardHeader title={t('track.names')} />
          <CardBody>
            <ul className="divide-y divide-border">
              {(names ?? []).map(n => (
                <li key={n.name}>
                  <button
                    type="button"
                    onClick={() =>
                      setSelected(s => (s === n.name ? null : n.name))
                    }
                    className={`flex w-full items-center justify-between gap-4 rounded px-2 py-2 text-left hover:bg-raised ${
                      selected === n.name ? 'bg-raised' : ''
                    }`}
                  >
                    <span className="min-w-0 flex-1 truncate font-mono text-sm text-fg">
                      {n.name}
                    </span>
                    <span className="tabular-nums text-sm text-fg-muted">
                      {formatNumber(n.total)}
                    </span>
                    {/* Zero distinct users is normal: an SDK sending
                        anonymously has no handle to count. Saying "0
                        users" next to 18k events would read as a bug,
                        so an absent count says nothing instead. */}
                    <span className="w-28 text-right text-xs text-fg-subtle">
                      {n.users > 0
                        ? t('track.users').replace('{n}', formatNumber(n.users))
                        : ''}
                    </span>
                    <span className="w-24 text-right text-xs text-fg-subtle">
                      {n.last_seen ? formatRelative(n.last_seen) : '—'}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
          </CardBody>
        </Card>
      )}

      {recent.length > 0 && (
        <Card>
          <CardHeader
            title={selected ?? t('track.recent')}
            subtitle={selected ? undefined : t('track.recentHint')}
          />
          <CardBody>
            <ul className="divide-y divide-border">
              {recent.map(e => (
                <li key={e.id} className="flex items-baseline gap-3 py-2">
                  <span className="w-24 shrink-0 text-right text-xs text-fg-subtle">
                    {formatRelative(e.occurred_at)}
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="font-mono text-sm text-fg">{e.name}</span>
                    {e.route && (
                      <span className="ml-2 font-mono text-xs text-fg-subtle">
                        {e.route}
                      </span>
                    )}
                    {/* The stored handle is a salted digest, not an
                        address. Showing a prefix identifies a session
                        across rows without pretending to name anyone. */}
                    {e.user_id && (
                      <span className="ml-2 font-mono text-xs text-accent">
                        {e.user_id.slice(0, 8)}…
                      </span>
                    )}
                  </span>
                </li>
              ))}
            </ul>
          </CardBody>
        </Card>
      )}
    </div>
  );
}
