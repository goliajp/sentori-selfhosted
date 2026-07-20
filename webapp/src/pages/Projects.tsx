// Projects admin — create / list / rename / delete.

import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';

import { api, Project, ProjectStats } from '../lib/api';
import {
  Button,
  Card,
  CardHeader,
  DataTable,
  EmptyState,
  ErrorBanner,
  PageHeader,
  Section,
} from '../components/ui';

export default function Projects() {
  const [rows, setRows] = useState<Project[]>([]);
  const [stats, setStats] = useState<Record<string, ProjectStats>>({});
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [name, setName] = useState('');
  const [slug, setSlug] = useState('');

  async function refresh() {
    setLoading(true);
    setError(null);
    try {
      const rows = await api.listProjects();
      setRows(rows);
      // Parallel per-project stats fetch
      const pairs = await Promise.all(
        rows.map(async p => {
          try {
            const s = await api.projectStats(p.id);
            return [p.id, s] as const;
          } catch {
            return [p.id, null] as const;
          }
        }),
      );
      setStats(
        Object.fromEntries(
          pairs.filter(([, v]) => v !== null) as [string, ProjectStats][],
        ),
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }
  useEffect(() => {
    refresh();
  }, []);

  async function create() {
    if (!name || !slug) return;
    try {
      await api.createProject({ name, slug });
      setName('');
      setSlug('');
      setShowCreate(false);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function rename(p: Project) {
    const next = prompt('New project name', p.name);
    if (!next || next === p.name) return;
    try {
      await api.renameProject(p.id, next);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function destroy(p: Project) {
    if (!confirm(`Delete project "${p.name}"? All events / issues / spans CASCADE-deleted.`))
      return;
    try {
      await api.deleteProject(p.id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="space-y-4">
      <PageHeader
        title="Projects"
        subtitle="One per app. Each project owns its own SDK tokens + push credentials."
        actions={
          <Button onClick={() => setShowCreate(true)}>+ New project</Button>
        }
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}
      {showCreate && (
        <Card>
          <CardHeader title="Create project" />
          <Section>
            <input
              className="w-full rounded border border-zinc-300 px-3 py-2 text-sm"
              placeholder="Display name (e.g. 'MyApp iOS')"
              value={name}
              onChange={e => {
                const v = e.target.value;
                setName(v);
                // Auto-suggest slug from name only if user hasn't
                // typed a custom slug already
                if (!slug || slug === slugify(name)) {
                  setSlug(slugify(v));
                }
              }}
            />
            <input
              className="mt-2 w-full rounded border border-zinc-300 px-3 py-2 text-sm font-mono"
              placeholder="Slug (e.g. 'myapp-ios')"
              value={slug}
              onChange={e => setSlug(e.target.value)}
            />
            <div className="mt-2 flex gap-2">
              <Button onClick={create}>Create</Button>
              <Button variant="secondary" onClick={() => setShowCreate(false)}>
                Cancel
              </Button>
            </div>
          </Section>
        </Card>
      )}
      <Card>
        <CardHeader title={`Projects (${rows.length})`} />
        <Section>
          {loading ? (
            <div className="py-8 text-center text-sm text-zinc-500">
              Loading…
            </div>
          ) : rows.length === 0 ? (
            <EmptyState
              title="No projects yet"
              hint="Create one to start ingesting events."
            />
          ) : (
            <DataTable
              columns={[
                { key: 'name', label: 'Name' },
                { key: 'slug', label: 'Slug' },
                { key: 'events', label: '24h events' },
                { key: 'active', label: 'Active' },
                { key: 'actions', label: '' },
              ]}
              rows={rows.map(p => ({
                key: p.id,
                name: (
                  <Link
                    to={`/projects/${p.id}/issues`}
                    className="text-emerald-600 hover:underline"
                  >
                    {p.name}
                  </Link>
                ),
                slug: <span className="font-mono text-xs">{p.slug}</span>,
                events: (
                  <span className="font-mono tabular-nums text-zinc-300">
                    {stats[p.id]
                      ? stats[p.id].events_24h.toLocaleString()
                      : '—'}
                  </span>
                ),
                active: (
                  <span className="font-mono tabular-nums text-orange-300">
                    {stats[p.id] ? stats[p.id].issues_active : '—'}
                  </span>
                ),
                actions: (
                  <div className="flex gap-1">
                    <Button size="sm" variant="secondary" onClick={() => rename(p)}>
                      Rename
                    </Button>
                    <Button size="sm" variant="danger" onClick={() => destroy(p)}>
                      Delete
                    </Button>
                  </div>
                ),
              }))}
            />
          )}
        </Section>
      </Card>
    </div>
  );
}

function slugify(s: string): string {
  return s
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 64);
}
