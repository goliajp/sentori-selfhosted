import { useEffect, useState } from 'react';
import { useT } from '../i18n';
import type { MessageKey } from '../i18n/en';
import { api, AlertRule, ApiError } from '../lib/api';
import {
  Badge,
  Button,
  Card,
  DataTable,
  ErrorBanner,
  PageHeader,
  formatRelative,
} from '../components/ui';

/**
 * What a rule watches for, said as a condition rather than as the enum
 * the column stores. `crash_free_drop` is the value; "Crash-free rate
 * drops" is the thing the person setting it up has in mind.
 */
const TRIGGER_KEYS: Record<string, MessageKey> = {
  new_issue: 'alerts.kindNewIssue',
  regression: 'alerts.kindRegression',
  event_count: 'alerts.kindEventCount',
  crash_free_drop: 'alerts.kindCrashFreeDrop',
};

function triggerLabel(kind: string, t: (k: MessageKey) => string): string {
  const key = TRIGGER_KEYS[kind];
  return key ? t(key) : kind;
}

export function AlertsPage() {
  const t = useT();
  const [alerts, setAlerts] = useState<AlertRule[] | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [refreshTok, setRefreshTok] = useState(0);
  const [showCreate, setShowCreate] = useState(false);
  const [name, setName] = useState('');
  const [throttle, setThrottle] = useState(10);

  async function create() {
    if (!name.trim()) return;
    try {
      await api.createAlert({
        name: name.trim(),
        enabled: true,
        trigger_kind: 'new_issue',
        trigger_config: {},
        filter_config: {},
        channels: {},
        throttle_minutes: throttle,
      });
      setName('');
      setShowCreate(false);
      setRefreshTok(t => t + 1);
    } catch (e) {
      setErr(String(e));
    }
  }

  useEffect(() => {
    api
      .listAlerts()
      .then(setAlerts)
      .catch((e: unknown) => {
        if (e instanceof ApiError) setErr(`${e.status}: ${e.body}`);
        else setErr(String(e));
      });
  }, [refreshTok]);

  async function editChannels(r: AlertRule) {
    const initial = JSON.stringify(r.channels ?? [], null, 2);
    const next = window.prompt(
      'Channels JSON. Example:\n[{"kind":"webhook","url":"https://hooks.slack.com/services/T.../...","secret":"opt"}]',
      initial,
    );
    if (next == null || next === initial) return;
    let parsed: unknown;
    try {
      parsed = JSON.parse(next);
    } catch {
      setErr(t('alerts.channelsInvalid'));
      return;
    }
    try {
      await api.patchAlert(r.id, { channels: parsed });
      setRefreshTok(t => t + 1);
    } catch (e) {
      setErr(String(e));
    }
  }

  async function fireTest(id: string) {
    try {
      const r = await api.fireTestAlert(id);
      const msg =
        r.errors.length > 0
          ? `Delivered ${r.delivered}; errors:\n${r.errors.join('\n')}`
          : `Delivered to ${r.delivered} channel(s).`;
      alert(msg);
    } catch (e) {
      setErr(String(e));
    }
  }

  async function deleteAlert(id: string) {
    if (!confirm(t('alerts.confirmDelete'))) return;
    try {
      await api.deleteAlert(id);
      setRefreshTok((t) => t + 1);
    } catch (e) {
      setErr(String(e));
    }
  }

  return (
    <div>
      <PageHeader
        title={t('alerts.title')}
        subtitle={t('alerts.subtitle')}
        action={
          <Button
            variant="primary"
            size="sm"
            onClick={() => setShowCreate(!showCreate)}
          >
            {showCreate ? t('action.cancel') : `+ ${t('alerts.newRule')}`}
          </Button>
        }
      />
      {err && <ErrorBanner>{err}</ErrorBanner>}

      {showCreate && (
        <Card className="mb-4 px-5 py-4">
          <p className="mb-2 text-xs text-fg-subtle">
            Minimal create — trigger_kind defaults to "issue_new" + filter
            / channels empty. Tune via PATCH /v1/alerts/:id afterward.
          </p>
          <div className="flex gap-2">
            <input
              className="flex-1 rounded border border-border-strong bg-surface px-3 py-2 text-sm"
              placeholder={t('alerts.namePlaceholder')}
              value={name}
              onChange={e => setName(e.target.value)}
            />
            <input
              type="number"
              className="w-24 rounded border border-border-strong bg-surface px-3 py-2 text-sm"
              value={throttle}
              onChange={e =>
                setThrottle(parseInt(e.target.value, 10) || 10)
              }
              title={t('alerts.throttle')}
            />
            <Button onClick={create} size="sm">{t('action.create')}</Button>
          </div>
        </Card>
      )}

      <Card>
        <DataTable
          rowKey={(r) => r.id}
          empty={t('alerts.empty')}
          rows={alerts ?? []}
          columns={[
            {
              key: 'enabled',
              label: t('alerts.on'),
              width: '5%',
              render: (r) => (
                <Badge tone={r.enabled && !r.muted ? 'ok' : 'neutral'}>
                  {r.muted ? 'muted' : r.enabled ? 'on' : 'off'}
                </Badge>
              ),
            },
            {
              key: 'name',
              label: t('alerts.name'),
              render: (r) => (
                <div>
                  <div className="font-medium text-fg">{r.name}</div>
                  <div className="font-mono text-xs text-fg-subtle">
                    {triggerLabel(r.trigger_kind, t)} ·{' '}
                    {t('alerts.throttleEvery').replace(
                      '{n}',
                      String(r.throttle_minutes),
                    )}{' · '}
                    {r.project_id
                      ? t('alerts.scopeProject')
                      : t('alerts.scopeWorkspace')}
                  </div>
                </div>
              ),
            },
            {
              key: 'last_fired_at',
              label: t('alerts.lastFired'),
              width: '15%',
              render: (r) =>
                r.last_fired_at ? (
                  <span className="text-xs text-fg-subtle">
                    {formatRelative(r.last_fired_at)}
                  </span>
                ) : (
                  <span className="text-xs text-fg-subtle">{t('alerts.never')}</span>
                ),
            },
            {
              key: 'channels-edit',
              label: '',
              width: '14%',
              render: (r) => (
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => editChannels(r)}
                >{t('alerts.channels')}</Button>
              ),
            },
            {
              key: 'fire',
              label: '',
              width: '14%',
              render: (r) => (
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={() => fireTest(r.id)}
                >{t('alerts.fireTest')}</Button>
              ),
            },
            {
              key: 'id',
              label: '',
              width: '10%',
              render: (r) => (
                <Button
                  variant="danger"
                  size="sm"
                  onClick={() => deleteAlert(r.id)}
                >{t('action.delete')}</Button>
              ),
            },
          ]}
        />
      </Card>
    </div>
  );
}
