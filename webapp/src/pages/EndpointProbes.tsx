// Synthetic HTTP endpoint monitor — add/remove/toggle probes.
//
// Probes are polled by a background worker (legacy ingest service
// — to be re-wired for v0.2 step K, not blocking ship).

import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';

import { api } from '../lib/api';
import {
  Badge,
  Button,
  Card,
  CardHeader,
  EmptyState,
  ErrorBanner,
  PageHeader,
  Section,
  formatRelative,
} from '../components/ui';

interface Probe {
  id: string;
  endpoint_url: string;
  method: string;
  expected_status: number;
  interval_sec: number;
  timeout_ms: number;
  enabled: boolean;
  created_at: string;
}

export default function EndpointProbes() {
  const { id: projectId } = useParams<{ id: string }>();
  const [rows, setRows] = useState<Probe[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showAdd, setShowAdd] = useState(false);
  const [url, setUrl] = useState('');
  const [method, setMethod] = useState('GET');
  const [interval, setInterval] = useState(60);

  async function refresh() {
    if (!projectId) return;
    setLoading(true);
    try {
      const r = await api.listEndpointProbes(projectId);
      setRows(r.probes);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    refresh();
  }, [projectId]);

  async function add() {
    if (!projectId || !url.trim()) return;
    try {
      await api.createEndpointProbe(projectId, {
        name: url.trim(),
        target_url: url.trim(),
        method,
        interval_sec: interval,
      });
      setUrl('');
      setShowAdd(false);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function toggle(p: Probe) {
    try {
      await api.setEndpointProbeEnabled(p.id, !p.enabled);
      setRows(rs =>
        rs.map(r => (r.id === p.id ? { ...r, enabled: !r.enabled } : r)),
      );
    } catch (e) {
      setError(String(e));
    }
  }

  async function destroy(p: Probe) {
    if (!confirm(`Delete probe for ${p.endpoint_url}?`)) return;
    try {
      await api.deleteEndpointProbe(p.id);
      setRows(rs => rs.filter(r => r.id !== p.id));
    } catch (e) {
      setError(String(e));
    }
  }

  if (!projectId) return <ErrorBanner>Project id missing</ErrorBanner>;

  return (
    <div className="space-y-4">
      <PageHeader
        title="Endpoint probes"
        subtitle="Synthetic HTTP monitor — periodic GET/POST against configured URLs."
        actions={<Button onClick={() => setShowAdd(true)}>+ Add probe</Button>}
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {showAdd && (
        <Card>
          <CardHeader title="New probe" />
          <Section>
            <input
              className="w-full rounded border border-zinc-300 px-3 py-2 text-sm"
              placeholder="https://api.example.com/health"
              value={url}
              onChange={e => setUrl(e.target.value)}
            />
            <div className="mt-2 flex gap-2">
              <select
                className="rounded border border-zinc-300 px-3 py-2 text-sm"
                value={method}
                onChange={e => setMethod(e.target.value)}
              >
                <option>GET</option>
                <option>HEAD</option>
                <option>POST</option>
              </select>
              <input
                type="number"
                className="rounded border border-zinc-300 px-3 py-2 text-sm w-24"
                value={interval}
                onChange={e => setInterval(parseInt(e.target.value, 10) || 60)}
                title="Interval (seconds)"
              />
              <span className="self-center text-xs text-zinc-500">sec</span>
            </div>
            <div className="mt-2 flex gap-2">
              <Button onClick={add}>Add</Button>
              <Button variant="secondary" onClick={() => setShowAdd(false)}>
                Cancel
              </Button>
            </div>
          </Section>
        </Card>
      )}

      <Card>
        <CardHeader title={`Probes (${rows.length})`} />
        <Section>
          {loading ? (
            <div className="py-8 text-center text-sm text-zinc-500">
              Loading…
            </div>
          ) : rows.length === 0 ? (
            <EmptyState
              title="No probes"
              hint="Add a URL to start synthetic monitoring."
            />
          ) : (
            <ul className="divide-y divide-zinc-800">
              {rows.map(p => (
                <li
                  key={p.id}
                  className="flex items-center justify-between gap-3 px-2 py-3"
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <Badge>{p.method}</Badge>
                      <span className="font-mono text-xs text-zinc-200 truncate">
                        {p.endpoint_url}
                      </span>
                      {p.enabled ? (
                        <Badge tone="ok">on</Badge>
                      ) : (
                        <Badge tone="neutral">off</Badge>
                      )}
                    </div>
                    <div className="mt-1 text-[10px] text-zinc-500">
                      expect {p.expected_status} · every {p.interval_sec}s ·
                      timeout {p.timeout_ms}ms · added{' '}
                      {formatRelative(p.created_at)}
                    </div>
                  </div>
                  <div className="flex gap-1">
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => toggle(p)}
                    >
                      {p.enabled ? 'Disable' : 'Enable'}
                    </Button>
                    <Button
                      size="sm"
                      variant="danger"
                      onClick={() => destroy(p)}
                    >
                      Delete
                    </Button>
                  </div>
                </li>
              ))}
            </ul>
          )}
        </Section>
      </Card>
    </div>
  );
}
