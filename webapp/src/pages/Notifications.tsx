// Per-user notification inbox.

import { useT } from '../i18n';
import type { MessageKey } from '../i18n/en';
import { api } from '../lib/api';
import { useAsyncData } from '../lib/useAsyncData';
import {
  Badge,
  Button,
  Card,
  CardBody,
  CardHeader,
  ErrorBanner,
  PageHeader,
  formatRelative,
} from '../components/ui';

interface Row {
  id: string;
  kind: string;
  payload: unknown;
  read_at: string | null;
  created_at: string;
}

/**
 * The one line a notification is about.
 *
 * The payload was being printed as raw JSON —
 * `{"title":"TypeError in CheckoutScreen"}` — which is the shape it is
 * stored in, not a sentence. Every kind carries a `title`; anything
 * that does not is a kind this function has not been taught, and
 * showing its keys is more honest than showing nothing.
 */
function summarise(payload: unknown): string {
  if (payload && typeof payload === 'object') {
    const p = payload as Record<string, unknown>;
    for (const k of ['title', 'message', 'name', 'summary']) {
      const v = p[k];
      if (typeof v === 'string') return v;
    }
    return Object.keys(p).join(', ');
  }
  return String(payload ?? '');
}

/** Notification kinds we have a word for; anything else shows raw. */
const KIND_KEYS: Record<string, MessageKey> = {
  issue_new: 'notifications.kindIssueNew',
  regression: 'notifications.kindRegression',
  quota: 'notifications.kindQuota',
};

function kindLabel(kind: string, t: (k: MessageKey) => string): string {
  const key = KIND_KEYS[kind];
  return key ? t(key) : kind;
}

export default function Notifications() {
  const t = useT();
  const { data, loading, error, setData } = useAsyncData(
    async (): Promise<Row[]> => (await api.listNotifications()).notifications,
    [],
    String,
  );
  const rows = data ?? [];

  async function readOne(id: string) {
    await api.markNotificationRead(id);
    setData(rs =>
      rs?.map(r =>
        r.id === id && !r.read_at
          ? { ...r, read_at: new Date().toISOString() }
          : r,
      ) ?? null,
    );
  }

  async function readAll() {
    await api.markAllNotificationsRead();
    const now = new Date().toISOString();
    setData(rs => rs?.map(r => (r.read_at ? r : { ...r, read_at: now })) ?? null);
  }

  const unread = rows.filter(r => !r.read_at).length;

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('notifications.title')}
        subtitle={
          unread > 0
            ? t('notifications.unread').replace('{n}', String(unread))
            : t('notifications.allRead')
        }
        actions={
          unread > 0 ? (
            <Button onClick={readAll} variant="secondary" size="sm">{t('action.markAllRead')}</Button>
          ) : null
        }
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Card>
        <CardHeader title={`${t('notifications.inbox')} (${rows.length})`} />
        <CardBody>
          {loading ? (
            <div className="py-8 text-center text-sm text-fg-subtle">
              {t('common.loading')}
            </div>
          ) : rows.length === 0 ? (
            <div className="py-8 text-center text-sm text-fg-subtle">
              {t('notifications.empty')}
            </div>
          ) : (
            <ul className="divide-y divide-border">
              {rows.map(n => (
                <li
                  key={n.id}
                  onClick={() => !n.read_at && readOne(n.id)}
                  className={`flex items-center justify-between gap-3 px-2 py-3 cursor-pointer ${
                    n.read_at ? 'opacity-60' : 'hover:bg-surface/40'
                  }`}
                >
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <Badge tone={n.read_at ? 'neutral' : 'info'}>
                        {kindLabel(n.kind, t)}
                      </Badge>
                      {!n.read_at && <span className="text-accent">●</span>}
                    </div>
                    <p className="mt-1 truncate text-sm text-fg">
                      {summarise(n.payload)}
                    </p>
                  </div>
                  <span className="text-xs text-fg-subtle w-24 text-right">
                    {formatRelative(n.created_at)}
                  </span>
                </li>
              ))}
            </ul>
          )}
        </CardBody>
      </Card>
    </div>
  );
}
