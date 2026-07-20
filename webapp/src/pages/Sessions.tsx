// Current user's active sessions. Show IP + UA + last-seen +
// revoke per row + revoke-all button.

import { useEffect, useState } from 'react';

import { api } from '../lib/api';
import {
  Button,
  Card,
  CardHeader,
  ErrorBanner,
  PageHeader,
  Section,
  formatRelative,
} from '../components/ui';

interface Row {
  id_hash_hex: string;
  created_at: string;
  last_used_at: string | null;
  expires_at: string;
  ip: string | null;
  user_agent: string | null;
}

export default function Sessions() {
  const [rows, setRows] = useState<Row[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [tick, setTick] = useState(0);

  useEffect(() => {
    api
      .listSessions()
      .then(r => setRows(r.sessions))
      .catch(e => setError(String(e)));
  }, [tick]);

  async function revoke(hash: string) {
    if (!confirm('Revoke this session?')) return;
    try {
      await api.revokeSession(hash);
      setTick(t => t + 1);
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="space-y-4">
      <PageHeader
        title="Active sessions"
        subtitle="Where your account is currently signed in. Revoke any you don't recognize."
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}
      <Card>
        <CardHeader title={`Sessions (${rows.length})`} />
        <Section>
          {rows.length === 0 ? (
            <div className="py-8 text-center text-sm text-zinc-500">
              No active sessions found.
            </div>
          ) : (
            <ul className="divide-y divide-zinc-800">
              {rows.map(s => (
                <li
                  key={s.id_hash_hex}
                  className="flex items-start justify-between gap-3 py-3"
                >
                  <div className="min-w-0 flex-1">
                    <div className="font-mono text-xs text-zinc-300">
                      {s.id_hash_hex.slice(0, 16)}…
                    </div>
                    <div className="mt-1 text-[10px] text-zinc-500">
                      {s.ip ?? 'IP unknown'} ·{' '}
                      {(s.user_agent ?? 'UA unknown').slice(0, 90)}
                    </div>
                    <div className="mt-1 text-[10px] text-zinc-500">
                      created {formatRelative(s.created_at)} · last seen{' '}
                      {s.last_used_at ? formatRelative(s.last_used_at) : 'never'}{' '}
                      · expires {formatRelative(s.expires_at)}
                    </div>
                  </div>
                  <Button
                    variant="danger"
                    size="sm"
                    onClick={() => revoke(s.id_hash_hex)}
                  >
                    Revoke
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </Section>
      </Card>
    </div>
  );
}
