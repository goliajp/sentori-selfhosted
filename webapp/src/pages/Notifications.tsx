// Per-user notification inbox.

import { useEffect, useState } from 'react';

import { api } from '../lib/api';
import {
  Badge,
  Button,
  Card,
  CardHeader,
  ErrorBanner,
  PageHeader,
  Section,
  formatRelative,
} from '../components/ui';

interface Row {
  id: string;
  kind: string;
  payload: unknown;
  read_at: string | null;
  created_at: string;
}

export default function Notifications() {
  const [rows, setRows] = useState<Row[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  async function refresh() {
    setLoading(true);
    try {
      const r = await api.listNotifications();
      setRows(r.notifications);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    refresh();
  }, []);

  async function readOne(id: string) {
    await api.markNotificationRead(id);
    setRows(rs =>
      rs.map(r =>
        r.id === id && !r.read_at
          ? { ...r, read_at: new Date().toISOString() }
          : r,
      ),
    );
  }

  async function readAll() {
    await api.markAllNotificationsRead();
    const now = new Date().toISOString();
    setRows(rs => rs.map(r => (r.read_at ? r : { ...r, read_at: now })));
  }

  const unread = rows.filter(r => !r.read_at).length;

  return (
    <div className="space-y-4">
      <PageHeader
        title="Notifications"
        subtitle={
          unread > 0
            ? `${unread} unread`
            : 'No unread notifications.'
        }
        actions={
          unread > 0 ? (
            <Button onClick={readAll} variant="secondary" size="sm">
              Mark all read
            </Button>
          ) : null
        }
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Card>
        <CardHeader title={`Inbox (${rows.length})`} />
        <Section>
          {loading ? (
            <div className="py-8 text-center text-sm text-zinc-500">
              Loading…
            </div>
          ) : rows.length === 0 ? (
            <div className="py-8 text-center text-sm text-zinc-500">
              No notifications.
            </div>
          ) : (
            <ul className="divide-y divide-zinc-800">
              {rows.map(n => (
                <li
                  key={n.id}
                  onClick={() => !n.read_at && readOne(n.id)}
                  className={`flex items-center justify-between gap-3 px-2 py-3 cursor-pointer ${
                    n.read_at ? 'opacity-60' : 'hover:bg-zinc-900/40'
                  }`}
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <Badge>{n.kind}</Badge>
                      {!n.read_at && (
                        <span className="text-emerald-400">●</span>
                      )}
                    </div>
                    <pre className="mt-1 overflow-x-auto whitespace-pre-wrap break-all text-[10px] font-mono text-zinc-500">
                      {JSON.stringify(n.payload)}
                    </pre>
                  </div>
                  <span className="text-xs text-zinc-500 w-24 text-right">
                    {formatRelative(n.created_at)}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </Section>
      </Card>
    </div>
  );
}
