// Projects admin — create / list / rename / delete.

import { useState } from 'react';
import { Link } from 'react-router-dom';

import { useT } from '../i18n';
import { api, Project, ProjectStats } from '../lib/api';
import { useAsyncData } from '../lib/useAsyncData';
import {
  Button,
  Card,
  CardBody,
  CardHeader,
  DataTable,
  EmptyState,
  ErrorBanner,
  PageHeader,
} from '../components/ui';

export default function Projects() {
  const t = useT();
  const [showCreate, setShowCreate] = useState(false);
  const [name, setName] = useState('');
  const [slug, setSlug] = useState('');

  const {
    data,
    loading,
    error,
    reload: refresh,
    setError,
  } = useAsyncData(
    async (): Promise<{
      rows: Project[];
      stats: Record<string, ProjectStats>;
    }> => {
      const rows = await api.listProjects();
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
      return {
        rows,
        stats: Object.fromEntries(
          pairs.filter(([, v]) => v !== null) as [string, ProjectStats][],
        ),
      };
    },
    [],
    String,
  );
  const rows = data?.rows ?? [];
  const stats = data?.stats ?? {};

  async function create() {
    if (!name || !slug) return;
    try {
      await api.createProject({ name, slug });
      setName('');
      setSlug('');
      setShowCreate(false);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function rename(p: Project) {
    const next = prompt(t('projects.newName'), p.name);
    if (!next || next === p.name) return;
    try {
      await api.renameProject(p.id, next);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function destroy(p: Project) {
    if (!confirm(t('projects.confirmDelete').replace('{name}', p.name)))
      return;
    try {
      await api.deleteProject(p.id);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('projects.title')}
        subtitle={t('projects.subtitle')}
        actions={
          <Button onClick={() => setShowCreate(true)}>+ {t('projects.create')}</Button>
        }
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}
      {showCreate && (
        <Card>
          <CardHeader title={t('projects.create')} />
          <CardBody>
            <input
              className="h-8 w-full rounded border border-border px-2.5 text-sm"
              placeholder={t('projects.nameHint')}
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
              className="mt-2 w-full rounded border border-border px-3 py-2 text-sm font-mono"
              placeholder={t('projects.slugHint')}
              value={slug}
              onChange={e => setSlug(e.target.value)}
            />
            <div className="mt-2 flex gap-2">
              <Button onClick={create}>{t('action.create')}</Button>
              <Button variant="secondary" onClick={() => setShowCreate(false)}>{t('action.cancel')}</Button>
            </div>
          </CardBody>
        </Card>
      )}
      <Card>
        <CardHeader title={`${t('projects.title')} (${rows.length})`} />
        <CardBody>
          {loading ? (
            <div className="py-8 text-center text-sm text-fg-subtle">
              Loading…
            </div>
          ) : rows.length === 0 ? (
            <EmptyState
              title={t('projects.empty')}
              hint={t('projects.emptyHint')}
            />
          ) : (
            <DataTable
              columns={[
                { key: 'name', label: t('projects.name') },
                { key: 'slug', label: t('projects.slug') },
                { key: 'events', label: t('projects.events24h') },
                { key: 'active', label: t('overview.activeIssues') },
                { key: 'actions', label: '' },
              ]}
              rows={rows.map(p => ({
                key: p.id,
                name: (
                  <Link
                    to={`/projects/${p.id}/issues`}
                    className="text-accent hover:underline"
                  >
                    {p.name}
                  </Link>
                ),
                slug: <span className="font-mono text-xs">{p.slug}</span>,
                events: (
                  <span className="font-mono tabular-nums text-fg-muted">
                    {stats[p.id]
                      ? stats[p.id].events_24h.toLocaleString()
                      : '—'}
                  </span>
                ),
                active: (
                  <span className="font-mono tabular-nums text-warn">
                    {stats[p.id] ? stats[p.id].issues_active : '—'}
                  </span>
                ),
                actions: (
                  <div className="flex gap-1">
                    <Button size="sm" variant="secondary" onClick={() => rename(p)}>
                      {t('action.rename')}
                    </Button>
                    <Button size="sm" variant="danger" onClick={() => destroy(p)}>
                      {t('action.delete')}
                    </Button>
                  </div>
                ),
              }))}
            />
          )}
        </CardBody>
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
