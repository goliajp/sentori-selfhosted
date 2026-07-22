// Cross-project search. Picks first project for now (multi-project
// picker lands when SaaS deployments need it).

import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';

import { useT } from '../i18n';
import { api, Project } from '../lib/api';
import {
  Badge,
  Card,
  CardBody,
  CardHeader,
  EmptyState,
  ErrorBanner,
  PageHeader,
  formatRelative,
} from '../components/ui';

interface IssueHit {
  id: string;
  error_type: string;
  message_sample: string;
  status: string;
  last_seen: string;
}

interface EventHit {
  id: string;
  issue_id: string;
  kind: string;
  release: string;
  environment: string;
  timestamp: string;
}

export default function Search() {
  const t = useT();
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectId, setProjectId] = useState('');
  const [q, setQ] = useState('');
  const [issues, setIssues] = useState<IssueHit[]>([]);
  const [events, setEvents] = useState<EventHit[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    api
      .listProjects()
      .then(p => {
        setProjects(p);
        if (p[0]) setProjectId(p[0].id);
      })
      .catch(e => setError(String(e)));
  }, []);

  // Clear results the moment the query stops qualifying, adjusted during
  // render rather than in an effect so it lands in the same commit.
  const searchable = Boolean(projectId) && q.trim().length >= 3;
  const [wasSearchable, setWasSearchable] = useState(searchable);
  if (searchable !== wasSearchable) {
    setWasSearchable(searchable);
    if (!searchable) {
      setIssues([]);
      setEvents([]);
    }
  }

  useEffect(() => {
    if (!searchable) return;
    const id = setTimeout(async () => {
      setLoading(true);
      try {
        const r = await api.searchProject(projectId, q.trim(), 50);
        setIssues(r.issues);
        setEvents(r.events);
      } catch (e) {
        setError(String(e));
      } finally {
        setLoading(false);
      }
    }, 250);
    return () => clearTimeout(id);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [projectId, q]);

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('search.title')}
        subtitle={t('search.subtitle')}
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Card>
        <CardBody>
          <div className="flex gap-2">
            <select
              value={projectId}
              onChange={e => setProjectId(e.target.value)}
              className="rounded border border-border-strong bg-surface px-3 py-2 text-sm"
            >
              {projects.map(p => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
            <input
              autoFocus
              value={q}
              onChange={e => setQ(e.target.value)}
              placeholder={t('search.placeholder')}
              className="flex-1 rounded border border-border-strong bg-surface px-3 py-2 text-sm"
            />
            {loading && (
              <span className="self-center text-xs text-fg-subtle">…</span>
            )}
          </div>
        </CardBody>
      </Card>

      {/* An empty screen is an invitation to act, not a blank. Before
          this, typing fewer than three characters left the page with a
          search box and nothing under it, which reads as broken rather
          than as waiting. */}
      {q.trim().length < 3 && issues.length === 0 && events.length === 0 && (
        <EmptyState title={t('search.empty')} hint={t('search.emptyHint')} />
      )}

      {issues.length > 0 && (
        <Card>
          <CardHeader title={`${t('issues.title')} (${issues.length})`} />
          <CardBody>
            <ul className="divide-y divide-border">
              {issues.map(i => (
                <li key={i.id} className="px-2 py-2">
                  <Link
                    to={`/projects/${projectId}/issues/${i.id}`}
                    className="block hover:bg-surface/40 -m-2 p-2 rounded"
                  >
                    <div className="flex items-center gap-2">
                      <Badge>{i.status}</Badge>
                      <span className="font-medium text-fg">
                        {i.error_type}
                      </span>
                      <span className="font-mono text-xs text-fg-subtle">
                        {formatRelative(i.last_seen)}
                      </span>
                    </div>
                    <p className="font-mono text-xs text-fg-subtle mt-1">
                      {i.message_sample.slice(0, 120)}
                    </p>
                  </Link>
                </li>
              ))}
            </ul>
          </CardBody>
        </Card>
      )}

      {events.length > 0 && (
        <Card>
          <CardHeader title={`${t('events.title')} (${events.length})`} />
          <CardBody>
            <ul className="space-y-1">
              {events.map(e => (
                <li
                  key={e.id}
                  className="flex items-center gap-2 text-xs"
                >
                  <Link
                    to={`/projects/${projectId}/issues/${e.issue_id}`}
                    className="font-mono text-accent hover:underline"
                  >
                    {e.kind}
                  </Link>
                  <span className="font-mono text-fg-muted">{e.release}</span>
                  <span className="font-mono text-fg-muted">
                    {e.environment}
                  </span>
                  <span className="font-mono text-fg-subtle">
                    {formatRelative(e.timestamp)}
                  </span>
                </li>
              ))}
            </ul>
          </CardBody>
        </Card>
      )}

      {q.trim().length >= 3 && !loading && !issues.length && !events.length && (
        <Card>
          <CardBody>
            <p className="py-8 text-center text-sm text-fg-subtle">
              No matches.
            </p>
          </CardBody>
        </Card>
      )}
    </div>
  );
}
