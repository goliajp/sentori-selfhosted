// Saved views — workspace-wide list + create + rename + delete.
//
// A saved view stores a query filter (status="active", release=..., etc)
// against one of {issues, events, spans, replays, metrics} so the user
// can re-run it with one click.

import { useEffect, useState } from 'react';
import { Link } from 'react-router-dom';

import { api, SavedView } from '../lib/api';
import {
  Badge,
  Button,
  Card,
  CardHeader,
  ErrorBanner,
  PageHeader,
  Section,
} from '../components/ui';

const TARGETS = ['issues', 'events', 'spans', 'replays', 'metrics'] as const;

export default function SavedViews() {
  const [target, setTarget] =
    useState<(typeof TARGETS)[number]>('issues');
  const [rows, setRows] = useState<SavedView[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState('');
  const [newPayload, setNewPayload] = useState('{}');

  async function refresh() {
    setLoading(true);
    setError(null);
    try {
      const r = await api.listSavedViews(target);
      setRows(r);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    refresh();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [target]);

  async function create() {
    if (!newName.trim()) return;
    let parsed: unknown;
    try {
      parsed = JSON.parse(newPayload);
    } catch {
      setError('Payload must be valid JSON');
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
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function rename(v: SavedView) {
    const next = prompt('New name', v.name);
    if (!next || next === v.name) return;
    try {
      await api.patchSavedView(v.id, { name: next });
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function destroy(v: SavedView) {
    if (!confirm(`Delete view "${v.name}"?`)) return;
    try {
      await api.deleteSavedView(v.id);
      await refresh();
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
        title="Saved views"
        subtitle="Stored query filters for issues / events / spans / replays / metrics."
        actions={
          <Button onClick={() => setShowCreate(true)}>+ Save view</Button>
        }
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Card>
        <CardHeader title="Target" />
        <div className="flex gap-1 p-2">
          {TARGETS.map(t => (
            <button
              key={t}
              onClick={() => setTarget(t)}
              className={`rounded px-3 py-1 text-xs font-mono ${
                target === t
                  ? 'bg-emerald-600 text-white'
                  : 'bg-zinc-800 text-zinc-300 hover:bg-zinc-700'
              }`}
            >
              {t}
            </button>
          ))}
        </div>
      </Card>

      {showCreate && (
        <Card>
          <CardHeader title={`Save new ${target} view`} />
          <Section>
            <input
              className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm"
              placeholder='Name (e.g. "active iOS prod")'
              value={newName}
              onChange={e => setNewName(e.target.value)}
            />
            <label className="mt-2 block text-xs text-zinc-500">
              Payload (JSON — filter shape; depends on target)
            </label>
            <textarea
              className="w-full h-32 rounded border border-zinc-700 bg-zinc-900 px-3 py-2 text-xs font-mono"
              value={newPayload}
              onChange={e => setNewPayload(e.target.value)}
            />
            <div className="mt-2 flex gap-2">
              <Button onClick={create}>Save</Button>
              <Button variant="secondary" onClick={() => setShowCreate(false)}>
                Cancel
              </Button>
            </div>
          </Section>
        </Card>
      )}

      <Card>
        <CardHeader title={`Views (${rows.length})`} />
        <Section>
          {loading ? (
            <div className="py-8 text-center text-sm text-zinc-500">
              Loading…
            </div>
          ) : rows.length === 0 ? (
            <div className="py-8 text-center text-sm text-zinc-500">
              No saved {target} views.
            </div>
          ) : (
            <ul className="divide-y divide-zinc-800">
              {rows.map(v => {
                const route = targetRoute(v);
                return (
                  <li
                    key={v.id}
                    className="flex items-center justify-between px-2 py-3"
                  >
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="text-sm font-medium text-zinc-100">
                          {v.name}
                        </span>
                        <Badge>{v.scope}</Badge>
                        {v.project_id ? (
                          <span className="font-mono text-[10px] text-zinc-500">
                            project {v.project_id.slice(0, 8)}…
                          </span>
                        ) : (
                          <Badge tone="neutral">workspace</Badge>
                        )}
                      </div>
                      <pre className="mt-1 overflow-x-auto whitespace-pre-wrap break-all text-[10px] font-mono text-zinc-500">
                        {JSON.stringify(v.payload)}
                      </pre>
                    </div>
                    <div className="flex items-center gap-1">
                      {route !== '#' && (
                        <Link
                          to={route}
                          className="rounded bg-zinc-800 px-3 py-1 text-xs text-zinc-300 hover:bg-zinc-700"
                        >
                          Open →
                        </Link>
                      )}
                      <Button
                        size="sm"
                        variant="secondary"
                        onClick={() => rename(v)}
                      >
                        Rename
                      </Button>
                      <Button
                        size="sm"
                        variant="danger"
                        onClick={() => destroy(v)}
                      >
                        Delete
                      </Button>
                    </div>
                  </li>
                );
              })}
            </ul>
          )}
        </Section>
      </Card>
    </div>
  );
}
