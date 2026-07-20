// Cross-project search. Picks first project for now (multi-project
// picker lands when SaaS deployments need it).

import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';

import { api, Project } from '../lib/api';
import {
  Badge,
  Card,
  CardHeader,
  ErrorBanner,
  PageHeader,
  Section,
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

  useEffect(() => {
    if (!projectId || q.trim().length < 3) {
      setIssues([]);
      setEvents([]);
      return;
    }
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
  }, [projectId, q]);

  return (
    <div className="space-y-4">
      <PageHeader
        title="Search"
        subtitle="LIKE search across issues + events for the selected project."
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Card>
        <Section>
          <div className="flex gap-2">
            <select
              value={projectId}
              onChange={e => setProjectId(e.target.value)}
              className="rounded border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm"
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
              placeholder="Type to search (≥3 chars)…"
              className="flex-1 rounded border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm"
            />
            {loading && (
              <span className="self-center text-xs text-zinc-500">…</span>
            )}
          </div>
        </Section>
      </Card>

      {issues.length > 0 && (
        <Card>
          <CardHeader title={`Issues (${issues.length})`} />
          <Section>
            <ul className="divide-y divide-zinc-800">
              {issues.map(i => (
                <li key={i.id} className="px-2 py-2">
                  <Link
                    to={`/projects/${projectId}/issues/${i.id}`}
                    className="block hover:bg-zinc-900/40 -m-2 p-2 rounded"
                  >
                    <div className="flex items-center gap-2">
                      <Badge>{i.status}</Badge>
                      <span className="font-medium text-zinc-100">
                        {i.error_type}
                      </span>
                      <span className="font-mono text-[10px] text-zinc-500">
                        {formatRelative(i.last_seen)}
                      </span>
                    </div>
                    <p className="font-mono text-[11px] text-zinc-500 mt-1">
                      {i.message_sample.slice(0, 120)}
                    </p>
                  </Link>
                </li>
              ))}
            </ul>
          </Section>
        </Card>
      )}

      {events.length > 0 && (
        <Card>
          <CardHeader title={`Events (${events.length})`} />
          <Section>
            <ul className="space-y-1">
              {events.map(e => (
                <li
                  key={e.id}
                  className="flex items-center gap-2 text-xs"
                >
                  <Link
                    to={`/projects/${projectId}/issues/${e.issue_id}`}
                    className="font-mono text-emerald-400 hover:underline"
                  >
                    {e.kind}
                  </Link>
                  <span className="font-mono text-zinc-400">{e.release}</span>
                  <span className="font-mono text-zinc-400">
                    {e.environment}
                  </span>
                  <span className="font-mono text-zinc-500">
                    {formatRelative(e.timestamp)}
                  </span>
                </li>
              ))}
            </ul>
          </Section>
        </Card>
      )}

      {q.trim().length >= 3 && !loading && !issues.length && !events.length && (
        <Card>
          <Section>
            <p className="py-8 text-center text-sm text-zinc-500">
              No matches.
            </p>
          </Section>
        </Card>
      )}
    </div>
  );
}
