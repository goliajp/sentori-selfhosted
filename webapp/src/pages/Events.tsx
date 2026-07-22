import { useEffect, useState } from 'react';
import { Link, useParams, useSearchParams } from 'react-router-dom';

import { useT } from '../i18n';
import { api, ApiError, EventRow } from '../lib/api';
import {
  Badge,
  Button,
  Card,
  CardHeader,
  DataTable,
  ErrorBanner,
  PageHeader,
  formatRelative,
} from '../components/ui';

export function EventsPage() {
  const t = useT();
  const { id: projectId } = useParams<{ id: string }>();
  const [search, setSearch] = useSearchParams();
  const issueFilter = search.get('issue_id') ?? '';
  const [events, setEvents] = useState<EventRow[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [saveMsg, setSaveMsg] = useState<string | null>(null);
  const [live, setLive] = useState(false);
  const [liveCount, setLiveCount] = useState(0);

  useEffect(() => {
    if (!projectId) return;
    api
      .listEvents(projectId, {
        limit: 100,
        issue_id: issueFilter || undefined,
      })
      .then(setEvents)
      .catch((e: unknown) => {
        if (e instanceof ApiError) setErr(`${e.status}: ${e.body}`);
        else setErr(String(e));
      });
  }, [projectId, issueFilter]);

  // Live tail subscription. Uses EventSource (cookie auth) — the
  // dashboard endpoint at /v1/projects/:id/events/_recent is
  // session-cookie-gated, not Bearer.
  useEffect(() => {
    if (!projectId || !live) return;
    const es = new EventSource(
      `/v1/projects/${projectId}/events/_recent`,
      { withCredentials: true },
    );
    es.addEventListener('event', (ev: MessageEvent) => {
      try {
        const data = JSON.parse(ev.data) as EventRow;
        if (issueFilter && data.issue_id !== issueFilter) return;
        setEvents(rows => {
          const next = [data, ...(rows ?? [])];
          return next.slice(0, 200);
        });
        setLiveCount(c => c + 1);
      } catch {
        /* ignore */
      }
    });
    es.onerror = () => {
      // Browser will auto-reconnect; just surface in UI.
      setErr('live tail disconnected (auto-retrying)…');
    };
    return () => es.close();
  }, [projectId, live, issueFilter]);

  async function saveView() {
    if (!projectId) return;
    const name = prompt(
      t('events.savedViewName'),
      `Events ${issueFilter ? 'for issue ' + issueFilter.slice(0, 8) : 'recent'} — ${new Date().toLocaleDateString()}`,
    );
    if (!name) return;
    try {
      await api.createSavedView({
        name,
        target: 'events',
        scope: 'workspace',
        project_id: projectId,
        payload: issueFilter ? { issue_id: issueFilter } : {},
      });
      setSaveMsg('Saved');
      setTimeout(() => setSaveMsg(null), 2000);
    } catch (e) {
      setSaveMsg(String(e).slice(0, 40));
    }
  }

  if (!projectId) return <div>no project id</div>;

  return (
    <div>
      <PageHeader
        title={t('events.title')}
        subtitle={t('events.subtitle')}
        action={
          <div className="flex items-center gap-2">
            {saveMsg && (
              <span className="font-mono text-xs text-fg-subtle">
                {saveMsg}
              </span>
            )}
            <Button
              onClick={() => {
                setLive(l => !l);
                setLiveCount(0);
              }}
              variant={live ? 'primary' : 'secondary'}
              size="sm"
            >
              {live ? `● ${t('events.live')} (${liveCount})` : `${t('events.live')} ○`}
            </Button>
            <Button onClick={saveView} variant="secondary" size="sm">{t('issues.saveFilter')}</Button>
          </div>
        }
      />

      {issueFilter && (
        <Card className="mb-4">
          <CardHeader title={t('events.filter')} />
          <div className="flex items-center justify-between p-3">
            <span className="font-mono text-xs">
              issue_id ={' '}
              <span className="text-accent">{issueFilter}</span>
            </span>
            <button
              onClick={() => {
                search.delete('issue_id');
                setSearch(search, { replace: true });
              }}
              className="rounded bg-raised px-2 py-1 text-xs text-fg-muted hover:bg-raised"
            >
              Clear ×
            </button>
          </div>
        </Card>
      )}

      {err && <ErrorBanner>{err}</ErrorBanner>}
      <Card>
        <DataTable
          rowKey={(r) => r.id}
          empty={t('events.empty')}
          rows={events ?? []}
          columns={[
            {
              key: 'kind',
              label: 'Kind',
              width: '10%',
              render: (r) => <Badge>{r.kind}</Badge>,
            },
            {
              key: 'platform',
              label: 'Plat',
              width: '10%',
              render: (r) => (
                <span className="font-mono text-xs text-fg-muted">
                  {r.platform}
                </span>
              ),
            },
            {
              key: 'release',
              label: 'Release',
              width: '20%',
              render: (r) => (
                <span className="font-mono text-xs text-fg-muted">
                  {r.release}
                </span>
              ),
            },
            {
              key: 'environment',
              label: 'Env',
              width: '12%',
              render: (r) => <Badge>{r.environment}</Badge>,
            },
            {
              key: 'issue_id',
              label: 'Issue',
              width: '20%',
              render: (r) => (
                <Link
                  to={`/projects/${projectId}/issues/${r.issue_id}`}
                  className="font-mono text-xs text-accent hover:underline"
                >
                  {r.issue_id.slice(0, 8)}…
                </Link>
              ),
            },
            {
              key: 'timestamp',
              label: 'When',
              width: '12%',
              render: (r) => (
                <span className="text-xs text-fg-subtle">
                  {formatRelative(r.timestamp)}
                </span>
              ),
            },
            {
              key: 'narrow',
              label: '',
              width: '8%',
              render: (r) => (
                <button
                  onClick={() => {
                    search.set('issue_id', r.issue_id);
                    setSearch(search, { replace: true });
                  }}
                  title={t('events.narrowToIssue')}
                  className="rounded bg-raised px-2 py-0.5 text-xs text-fg-muted hover:bg-raised"
                >
                  {t('events.narrowShort')}
                </button>
              ),
            },
          ]}
        />
      </Card>
    </div>
  );
}
