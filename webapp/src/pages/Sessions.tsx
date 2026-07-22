// Current user's active sessions. Show IP + UA + last-seen +
// revoke per row + revoke-all button.

import { useEffect, useState } from 'react';

import { useT } from '../i18n';
import { api } from '../lib/api';
import {
  Button,
  Card,
  CardBody,
  CardHeader,
  ErrorBanner,
  PageHeader,
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
  const t = useT();
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
    if (!confirm(t('sessions.confirmRevoke'))) return;
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
        title={t('sessions.title')}
        subtitle={t('sessions.subtitle')}
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}
      <Card>
        <CardHeader title={`${t('sessions.title')} (${rows.length})`} />
        <CardBody>
          {rows.length === 0 ? (
            <div className="py-8 text-center text-sm text-fg-subtle">
              No active sessions found.
            </div>
          ) : (
            <ul className="divide-y divide-border">
              {rows.map(s => (
                <li
                  key={s.id_hash_hex}
                  className="flex items-start justify-between gap-3 py-3"
                >
                  <div className="min-w-0 flex-1">
                    <div className="font-mono text-xs text-fg-muted">
                      {s.id_hash_hex.slice(0, 16)}…
                    </div>
                    <div className="mt-1 text-xs text-fg-subtle">
                      {s.ip ?? t('sessions.ipUnknown')} ·{' '}
                      {(s.user_agent ?? t('sessions.deviceUnknown')).slice(0, 90)}
                    </div>
                    <div className="mt-1 text-xs text-fg-subtle">
                      {t('sessions.created').replace(
                        '{when}',
                        formatRelative(s.created_at),
                      )}{' · '}
                      {t('sessions.lastSeen').replace(
                        '{when}',
                        s.last_used_at
                          ? formatRelative(s.last_used_at)
                          : t('sessions.never'),
                      )}{' · '}
                      {t('sessions.expires').replace(
                        '{when}',
                        formatRelative(s.expires_at),
                      )}
                    </div>
                  </div>
                  <Button
                    variant="danger"
                    size="sm"
                    onClick={() => revoke(s.id_hash_hex)}
                  >{t('action.revoke')}</Button>
                </li>
              ))}
            </ul>
          )}
        </CardBody>
      </Card>
    </div>
  );
}
