// Saved views — workspace-wide list + create + rename + delete.
//
// A saved view stores a query filter (status="active", release=..., etc)
// against one of {issues, events, spans, replays, metrics} so the user
// can re-run it with one click.

import { useState } from 'react';
import { Link } from 'react-router-dom';

import { useT } from '../i18n';
import { api, SavedView } from '../lib/api';
import { useAsyncData } from '../lib/useAsyncData';
import {
  Badge,
  Button,
  Card,
  CardBody,
  CardHeader,
  ErrorBanner,
  PageHeader,
} from '../components/ui';

const TARGETS = ['issues', 'events', 'spans', 'replays', 'metrics'] as const;

export default function SavedViews() {
  const t = useT();
  const [target, setTarget] =
    useState<(typeof TARGETS)[number]>('issues');
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState('');
  const [newPayload, setNewPayload] = useState('{}');

  const {
    data,
    loading,
    error,
    reload: refresh,
    setError,
  } = useAsyncData(() => api.listSavedViews(target), [target], String);
  const rows = data ?? [];

  async function create() {
    if (!newName.trim()) return;
    let parsed: unknown;
    try {
      parsed = JSON.parse(newPayload);
    } catch {
      setError(t('common.jsonInvalid'));
      return;
    }
    try {
      await api.createSavedView({
        name: newName.trim(),
        target,
        scope: 'workspace',
        payload: parsed,
      });
      setNewName('');
      setNewPayload('{}');
      setShowCreate(false);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function rename(v: SavedView) {
    const next = prompt('New name', v.name);
    if (!next || next === v.name) return;
    try {
      await api.patchSavedView(v.id, { name: next });
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function destroy(v: SavedView) {
    if (!confirm(`Delete view "${v.name}"?`)) return;
    try {
      await api.deleteSavedView(v.id);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  function targetRoute(v: SavedView): string {
    if (!v.project_id) return '#';
    const payload = (v.payload ?? {}) as Record<string, string | undefined>;
    const params = new URLSearchParams();
    // Status filter — Issues page reads `?status=` to seed the
    // tab. "all" is encoded as no param (legacy compatible).
    if (payload.status && payload.status !== 'all') {
      params.set('status', String(payload.status));
    }
    const qs = params.toString();
    const suffix = qs ? `?${qs}` : '';
    if (v.target === 'issues') {
      return `/projects/${v.project_id}/issues${suffix}`;
    }
    if (v.target === 'events') {
      if (payload.issue_id) {
        params.set('issue_id', String(payload.issue_id));
      }
      const q2 = params.toString();
      return `/projects/${v.project_id}/events${q2 ? `?${q2}` : ''}`;
    }
    if (v.target === 'spans') {
      return `/projects/${v.project_id}/traces`;
    }
    if (v.target === 'replays') {
      return `/projects/${v.project_id}/replays`;
    }
    if (v.target === 'metrics') {
      return `/projects/${v.project_id}/metrics`;
    }
    return '#';
  }

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('savedViews.title')}
        subtitle={t('savedViews.subtitle')}
        actions={
          <Button onClick={() => setShowCreate(true)}>{'+ ' + t('savedViews.title')}</Button>
        }
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Card>
        <CardHeader title={t('savedViews.target')} />
        <div className="flex gap-1 p-2">
          {TARGETS.map(t => (
            <button
              key={t}
              onClick={() => setTarget(t)}
              className={`rounded px-3 py-1 text-xs font-mono ${
                target === t
                  ? 'bg-accent text-white'
                  : 'bg-raised text-fg-muted hover:bg-raised'
              }`}
            >
              {t}
            </button>
          ))}
        </div>
      </Card>

      {showCreate && (
        <Card>
          <CardHeader title={t('savedViews.saveNew')} />
          <CardBody>
            <input
              className="w-full rounded border border-border-strong bg-surface px-3 py-2 text-sm"
              placeholder={t('savedViews.namePlaceholder')}
              value={newName}
              onChange={e => setNewName(e.target.value)}
            />
            <label className="mt-2 block text-xs text-fg-subtle">
              Payload (JSON — filter shape; depends on target)
            </label>
            <textarea
              className="w-full h-32 rounded border border-border-strong bg-surface px-3 py-2 text-xs font-mono"
              value={newPayload}
              onChange={e => setNewPayload(e.target.value)}
            />
            <div className="mt-2 flex gap-2">
              <Button onClick={create}>{t('action.save')}</Button>
              <Button variant="secondary" onClick={() => setShowCreate(false)}>{t('action.cancel')}</Button>
            </div>
          </CardBody>
        </Card>
      )}

      <Card>
        <CardHeader title={`${t('savedViews.views')} (${rows.length})`} />
        <CardBody>
          {loading ? (
            <div className="py-8 text-center text-sm text-fg-subtle">
              Loading…
            </div>
          ) : rows.length === 0 ? (
            <div className="py-8 text-center text-sm text-fg-subtle">
              No saved {target} views.
            </div>
          ) : (
            <ul className="divide-y divide-border">
              {rows.map(v => {
                const route = targetRoute(v);
                return (
                  <li
                    key={v.id}
                    className="flex items-center justify-between px-2 py-3"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-fg">
                          {v.name}
                        </span>
                        <Badge>{v.scope}</Badge>
                        {v.project_id ? (
                          <span className="font-mono text-xs text-fg-subtle">
                            project {v.project_id.slice(0, 8)}…
                          </span>
                        ) : (
                          <Badge tone="neutral">workspace</Badge>
                        )}
                      </div>
                      <pre className="mt-1 overflow-x-auto whitespace-pre-wrap break-all text-xs font-mono text-fg-subtle">
                        {JSON.stringify(v.payload)}
                      </pre>
                    </div>
                    <div className="flex items-center gap-1">
                      {route !== '#' && (
                        <Link
                          to={route}
                          className="rounded bg-raised px-3 py-1 text-xs text-fg-muted hover:bg-raised"
                        >
                          {t('savedViews.open')} →
                        </Link>
                      )}
                      <Button
                        size="sm"
                        variant="secondary"
                        onClick={() => rename(v)}
                      >{t('action.rename')}</Button>
                      <Button
                        size="sm"
                        variant="danger"
                        onClick={() => destroy(v)}
                      >{t('action.delete')}</Button>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </CardBody>
      </Card>
    </div>
  );
}
