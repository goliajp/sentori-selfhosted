import { Action } from '../../shared/design/controls';
import { Checkbox } from "../../shared/design/Checkbox";
import { providerLabel } from '../lib/technical-labels';
import { WorkbenchEmpty } from '../components/WorkbenchEmpty';
import { useRef, useState, type ReactNode } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { Plus, Send } from 'lucide-react';
import { Heading, Surface } from '../../shared/design/Surface';
import { Metric } from '../../shared/design/Metric';
import { Tabs } from '../../shared/design/Tabs';
import {
  Badge,
  Button,
  Field,
  Input,
  Textarea,
} from '../../shared/design/controls';
import { FileInput } from '../../shared/design/FileInput';
import { useSharedT } from '../i18n/shared';
import { Dialog } from '../../shared/design/Dialog';
import { useShell } from '../App';
import { DataTable, ErrorBanner, Select, TimeAgo } from '../components/ui';
import { useT, useLocale } from '../i18n';
import type { MessageKey } from '../i18n/en';
import {
  api,
  ApiError,
  type PushSend,
  type PushDevice,
  type PushCredential,
  type AudienceRequest,
} from '../lib/api';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';
import { formatApiError } from '../lib/useAsyncData';
import { usePushCommand } from '../lib/usePushCommand';
import { commandId } from '../lib/team-contract';
import {
  PROVIDER_SPECS,
  recognise,
  type Provider,
} from '../lib/push-credentials';
import { PROVIDER_FIELDS } from '../lib/push-fields';
import {
  blank,
  OPS,
  toRequest,
  valueOnly,
  type Condition,
  type Source,
  type Op,
} from '../lib/push-audience';
import { IntegrateSection, ProviderGuide } from './PushHelp';

const SECTIONS = [
  'delivery',
  'audience',
  'devices',
  'credentials',
  'integrate',
] as const;
type Section = (typeof SECTIONS)[number];
const PROVIDERS = ['apns', 'fcm', 'webpush', 'hcm', 'mipush'];
const PAGE = 25;
function ReadState({
  loading,
  error,
  children,
}: {
  loading: boolean;
  error: string | null;
  children?: ReactNode;
}) {
  const t = useT();
  return (
    <>
      {loading && <p role="status">{t('shell.loading')}</p>}
      {error && <ErrorBanner reason={error}>{t('team.failed')}</ErrorBanner>}
      {children}
    </>
  );
}
function Pager({
  offset,
  total,
  move,
}: {
  offset: number;
  total: number;
  move: (offset: number) => void;
}) {
  const t = useT();
  return (
    <div className="push-pagination">
      <span>
        {total ? offset + 1 : 0}–{Math.min(offset + PAGE, total)} / {total}
      </span>
      <Button
        small
        disabled={!offset}
        onClick={() => move(Math.max(0, offset - PAGE))}
      >
        {t('queue.previous')}
      </Button>
      <Button
        small
        disabled={offset + PAGE >= total}
        onClick={() => move(offset + PAGE)}
      >
        {t('queue.next')}
      </Button>
    </div>
  );
}
function CommandError({
  command,
}: {
  command: ReturnType<typeof usePushCommand>;
}) {
  const t = useT();
  return (
    <>
      {command.error && (
        <ErrorBanner reason={command.error}>{t('team.failed')}</ErrorBanner>
      )}
      {command.uncertain && <p role="status">{t('team.uncertain')}</p>}
    </>
  );
}
function Status({ send }: { send: PushSend }) {
  const t = useT(),
    state = send.deliveredAt ? 'acknowledged' : send.displayState;
  return (
    <Badge
      tone={
        state === 'acknowledged' || state === 'sent'
          ? 'success'
          : state === 'failed'
            ? 'danger'
            : state === 'uncertain'
              ? 'warning'
              : 'neutral'
      }
    >
      {t(`pushOps.state.${state}` as MessageKey)}
    </Badge>
  );
}
export default function PushPage() {
  const { activeProject } = useShell(),
    t = useT();
  return activeProject ? (
    <Push key={activeProject.id} project={activeProject.id} />
  ) : (
    <div className="production-page"><Heading title={t('nav.push')}/><Surface><WorkbenchEmpty title={t('team.chooseProject')} action={<Link className="text-link" to="/projects">{t('nav.projects')}</Link>}/></Surface></div>
  );
}
function Push({ project }: { project: string }) {
  const t = useT(),
    [params, setParams] = useSearchParams();
  const asked = params.get('tab') as Section,
    section = SECTIONS.includes(asked) ? asked : 'delivery';
  function go(tab: string) {
    setParams((old) => {
      const next = new URLSearchParams(old);
      next.set('tab', tab);
      return next;
    });
  }
  return (
    <div className="production-page team-page push-page" data-push-section={section}>
      <Heading title={t('nav.push')} description={t('pushOps.description')} />
      <Tabs
        variant="line"
        label={t('nav.push')}
        items={SECTIONS.map((s) => [s, t(`push.section.${s}`)] as const)}
        value={section}
        onChange={go}
      >
        {section === 'delivery' ? (
          <Delivery project={project} onGo={go} />
        ) : section === 'devices' ? (
          <Devices project={project} />
        ) : section === 'credentials' ? (
          <Credentials project={project} />
        ) : section === 'audience' ? (
          <Audience project={project} />
        ) : (
          <IntegrateSection project={project} />
        )}
      </Tabs>
    </div>
  );
}
function Delivery({
  project,
  onGo,
}: {
  project: string;
  onGo: (tab: string) => void;
}) {
  const t = useT(),
    [params, setParams] = useSearchParams(),
    [testing, setTesting] = useState(false),
    [selected, setSelected] = useState<string | null>(null),
    [bulk, setBulk] = useState<PushSend[] | null>(null);
  const status = params.get('status') ?? '',
    provider = params.get('provider') ?? '',
    search = params.get('q') ?? '',
    offset = Math.max(0, Number(params.get('offset')) || 0);
  const list = useScopedAsyncData(
    JSON.stringify([project, status, provider, search, offset]),
    () => api.pushSends(project, PAGE, status, offset, search, provider),
  );
  const health = useScopedAsyncData(project, () => api.pushHealth(project));
  const reload = () => {
    list.reload();
    health.reload();
  };
  const filter = (key: string, value: string) =>
    setParams(
      (old) => {
        const next = new URLSearchParams(old);
        if (value) next.set(key, value);
        else next.delete(key);
        if (key !== 'offset') next.delete('offset');
        return next;
      },
      { replace: true },
    );
  const failed =
    list.data?.sends.filter((s) => s.displayState === 'failed') ?? [];
  return (
    <div className="push-stack">
      <div className="push-metrics">
        <Metric
          label={t('pushOps.accepted24h')}
          value={health.data?.sent24h ?? '—'}
        />
        <Metric
          label={t('push.failed24h')}
          note={t('pushOps.stoppedKinds')}
          value={health.data?.failed24h ?? '—'}
        />
        <Metric
          label={t('pushOps.state.queued')}
          value={health.data?.queued ?? '—'}
        />
        <Metric
          label={t('pushOps.liveDevices')}
          value={health.data?.liveTokens ?? '—'}
        />
      </div>
      {health.error && (
        <ErrorBanner reason={health.error}>{t('team.failed')}</ErrorBanner>
      )}
      <Readiness project={project} onGo={onGo} />
      <Surface
        title={t('push.section.delivery')}
        action={
          <Button data-push-test variant="primary" onClick={() => setTesting(true)}>
            <Send size={15} />
            {t('push.testSend')}
          </Button>
        }
      >
        <div className="push-toolbar">
          <Input
            aria-label={t('pushOps.search')}
            placeholder={t('pushOps.search')}
            value={search}
            maxLength={200}
            onChange={(e) => filter('q', e.target.value)}
          />
          <Select
            aria-label={t('push.statusAny')}
            value={status}
            onChange={(e) => filter('status', e.target.value)}
          >
            <option value="">{t('push.statusAny')}</option>
            {[
              'queued',
              'sending',
              'sent',
              'failed',
              'uncertain',
              'cancelled',
            ].map((s) => (
              <option key={s} value={s}>
                {t(`pushOps.state.${s}` as MessageKey)}
              </option>
            ))}
          </Select>
          <Select
            aria-label={t('pushOps.provider')}
            value={provider}
            onChange={(e) => filter('provider', e.target.value)}
          >
            <option value="">{t('pushOps.allProviders')}</option>
            {PROVIDERS.map((p) => (
              <option key={p} value={p}>{providerLabel(p)}</option>
            ))}
          </Select>
          <Button onClick={reload}>{t('team.refresh')}</Button>
          <Button disabled={!failed.length} onClick={() => setBulk(failed)}>
            {t('pushOps.retryVisible')}
          </Button>
        </div>
        <ReadState loading={list.loading} error={list.error} />
        {list.data && (
          <>
            <DataTable
              appearance="design"
              rows={list.data.sends}
              columns={[
                {
                  key: 'message',
                  label: t('push.testSubject'),
                  render: (s) => (
                    <Action
                      className="text-link text-left"
                      onClick={() => setSelected(s.id)}
                    >
                      {s.payload?.title || s.id.slice(0, 8)}
                    </Action>
                  ),
                },
                {
                  key: 'provider',
                  label: t('pushOps.provider'),
                  render: (s) => <code>{providerLabel(s.provider)}</code>,
                },
                {
                  key: 'status',
                  label: t('pushOps.status'),
                  render: (s) => <Status send={s} />,
                },
                {
                  key: 'attempts',
                  label: t('pushOps.retries'),
                  render: (s) => s.retry_count,
                },
                {
                  key: 'at',
                  label: t('pushOps.created'),
                  render: (s) => <TimeAgo iso={s.created_at} />,
                },
                {
                  key: 'open',
                  label: '',
                  render: (s) => (
                    <Button small onClick={() => setSelected(s.id)}>
                      {t('pushOps.details')}
                    </Button>
                  ),
                },
              ]}
            />
            {!list.data.sends.length && (
              <p className="muted py-6">{t('pushOps.noSends')}</p>
            )}
            <Pager
              offset={offset}
              total={list.data.total}
              move={(n) => filter('offset', String(n))}
            />
          </>
        )}
      </Surface>
      {testing && (
        <TestDialog
          project={project}
          onClose={() => setTesting(false)}
          saved={reload}
        />
      )}{' '}
      {selected && (
        <SendDialog
          project={project}
          id={selected}
          onClose={() => setSelected(null)}
          saved={reload}
        />
      )}{' '}
      {bulk && (
        <BulkDialog
          project={project}
          rows={bulk}
          onClose={() => setBulk(null)}
          saved={() => {
            setBulk(null);
            reload();
          }}
        />
      )}
    </div>
  );
}
function TestDialog({
  project,
  onClose,
  saved,
}: {
  project: string;
  onClose: () => void;
  saved: () => void;
}) {
  const t = useT(),
    [offset, setOffset] = useState(0),
    devices = useScopedAsyncData(`${project}:${offset}`, () =>
      api.pushDevices(project, PAGE, 'live', offset),
    );
  const [device, setDevice] = useState<PushDevice | null>(null),
    [title, setTitle] = useState(''),
    [body, setBody] = useState(''),
    [review, setReview] = useState(false),
    [sent, setSent] = useState<string | null>(null);
  const command = usePushCommand(project, (r) => {
    setSent(r.sendId!);
    saved();
  });
  return (
    <Dialog
      open
      title={t('push.testTitle')}
      closeLabel={t('common.close')}
      onClose={() => {
        if (!command.locked) onClose();
      }}
      footer={
        <>
          <Button disabled={command.locked} onClick={onClose}>
            {t('common.close')}
          </Button>
          {!sent && (
            <Button
              variant="primary"
              busy={command.busy}
              disabled={!device || !title.trim()}
              onClick={() =>
                command.uncertain
                  ? void command.run()
                  : review
                    ? void command.run({
                        type: 'testSend',
                        deviceId: device!.id,
                        title,
                        body,
                      })
                    : setReview(true)
              }
            >
              {command.uncertain
                ? t('common.retry')
                : review
                  ? t('pushOps.confirmSend')
                  : t('pushOps.review')}
            </Button>
          )}
        </>
      }
    >
      {sent ? (
        <p role="status">
          {t('pushOps.queuedResult')} <code>{sent}</code>
        </p>
      ) : review ? (
        <div className="push-stack">
          <p>{t('pushOps.reviewDevice')}</p>
          <code>
            {device?.id} · {providerLabel(device?.provider)}
          </code>
          <strong>{title}</strong>
          <p className="whitespace-pre-wrap break-words">{body}</p>
          <Button disabled={command.locked} onClick={() => setReview(false)}>
            {t('monitor.edit')}
          </Button>
        </div>
      ) : (
        <fieldset className="form-stack min-w-0" disabled={command.locked}>
          <ReadState loading={devices.loading} error={devices.error} />
          <Field label={t('push.device')}>
            <Select
              value={device?.id ?? ''}
              onChange={(e) =>
                setDevice(
                  devices.data?.devices.find((d) => d.id === e.target.value) ??
                    null,
                )
              }
            >
              <option value="">{t('pushOps.chooseDevice')}</option>
              {devices.data?.devices.map((d) => (
                <option key={d.id} value={d.id}>
                  {providerLabel(d.provider)} · {d.tokenTail || d.id.slice(0, 8)}
                </option>
              ))}
            </Select>
          </Field>
          {devices.data && (
            <Pager
              offset={offset}
              total={devices.data.total}
              move={(n) => {
                setDevice(null);
                setOffset(n);
              }}
            />
          )}
          <Field label={t('push.testSubject')}>
            <Input
              maxLength={120}
              required
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
          </Field>
          <Field label={t('push.testBody')}>
            <Textarea
              maxLength={3000}
              value={body}
              onChange={(e) => setBody(e.target.value)}
            />
          </Field>
        </fieldset>
      )}
      <CommandError command={command} />
    </Dialog>
  );
}
function SendDialog({
  project,
  id,
  onClose,
  saved,
}: {
  project: string;
  id: string;
  onClose: () => void;
  saved: () => void;
}) {
  const t = useT(),
    detail = useScopedAsyncData(`${project}:${id}`, () =>
      api.pushSendDetail(project, id),
    ),
    [ack, setAck] = useState(false);
  const command = usePushCommand(project, () => {
    detail.reload();
    saved();
  });
  const s = detail.data?.send;
  return (
    <Dialog
      open
      title={t('pushOps.details')}
      closeLabel={t('common.close')}
      onClose={() => {
        if (!command.locked) onClose();
      }}
      footer={
        <>
          <Button onClick={detail.reload}>{t('team.refresh')}</Button>
          {s?.status === 'failed' && (
            <Button
              busy={command.busy}
              disabled={s.displayState === 'uncertain' && !ack}
              onClick={() =>
                void command.run({
                  type: 'retrySend',
                  id,
                  expectedVersion: s.version,
                  acknowledgeDuplicate: ack,
                })
              }
            >
              {command.uncertain ? t('common.retry') : t('push.retry')}
            </Button>
          )}
        </>
      }
    >
      <ReadState loading={detail.loading} error={detail.error} />
      {s && (
        <div className="push-stack">
          <Status send={s} />
          <h3>{s.payload?.title}</h3>
          <p className="whitespace-pre-wrap break-words">{s.payload?.body}</p>
          <code className="break-all">{s.id}</code>
          <p>
            {t('push.device')}: <code>{s.token_id}</code>
          </p>
          <p>
            {t('pushOps.provider')}: {providerLabel(s.provider)}
          </p>
          <p>
            {t('pushOps.credential')}: <code>{s.credentialId ?? '—'}</code>
          </p>
          <p>
            {t('pushOps.accepted')}:{' '}
            {s.sent_at ? <TimeAgo iso={s.sent_at} /> : '—'}
          </p>
          <p>
            {t('pushOps.receipt')}:{' '}
            {s.deliveredAt ? (
              <TimeAgo iso={s.deliveredAt} />
            ) : (
              t('pushOps.noReceipt')
            )}
          </p>
          {s.error && <code>{s.error}</code>}
          <h3>{t('pushOps.attempts')}</h3>
          {detail.data!.attempts.map((a) => (
            <div className="push-attempt" key={a.id}>
              <TimeAgo iso={a.at} />
              <code>{a.outcome}</code>
              <span>
                {a.httpStatus || '—'} · {a.durationMs ?? '—'} ms
              </span>
            </div>
          ))}
          {!detail.data!.attempts.length && <p>{t('pushOps.noAttempts')}</p>}
          {s.displayState === 'uncertain' && (
            <Checkbox checked={ack} disabled={command.locked} onChange={nextChecked => setAck(nextChecked)} label={t('pushOps.duplicateAck')}/>
          )}
        </div>
      )}
      <CommandError command={command} />
    </Dialog>
  );
}
function BulkDialog({
  project,
  rows,
  onClose,
  saved,
}: {
  project: string;
  rows: PushSend[];
  onClose: () => void;
  saved: () => void;
}) {
  const t = useT(),
    command = usePushCommand(project, saved);
  return (
    <Dialog
      open
      title={t('pushOps.retryVisible')}
      closeLabel={t('common.close')}
      onClose={() => {
        if (!command.locked) onClose();
      }}
      footer={
        <Button
          variant="primary"
          busy={command.busy}
          onClick={() =>
            void command.run({
              type: 'retryFailed',
              ids: rows.map((r) => r.id),
            })
          }
        >
          {command.uncertain ? t('common.retry') : t('pushOps.confirmRetry')}
        </Button>
      }
    >
      <p>{t('pushOps.bulkHint', { count: String(rows.length) })}</p>
      <ul>
        {rows.map((r) => (
          <li key={r.id}>
            {r.payload?.title} · <code>{r.id.slice(0, 8)}</code>
          </li>
        ))}
      </ul>
      <CommandError command={command} />
    </Dialog>
  );
}
function Devices({ project }: { project: string }) {
  const t = useT(),
    [params, setParams] = useSearchParams(),
    scope = params.get('scope') === 'all' ? 'all' : 'live',
    search = params.get('deviceQuery') ?? '', provider = params.get('deviceProvider') ?? '',
    offset = Math.max(0, Number(params.get('deviceOffset')) || 0);
  const list = useScopedAsyncData(JSON.stringify([project,scope,offset,search,provider]), () =>
      api.pushDevices(project, PAGE, scope, offset, search, provider),
    ),
    [selected, setSelected] = useState<PushDevice | null>(null);
  const update = (key: string, value: string) =>
    setParams(
      (old) => {
        const next = new URLSearchParams(old);
        next.set(key, value);
        if (key !== 'deviceOffset') next.delete('deviceOffset');
        return next;
      },
      { replace: true },
    );
  return (
    <Surface
      title={t('push.devicesTitle')}
      action={<Button onClick={list.reload}>{t('team.refresh')}</Button>}
    >
      <div className="push-toolbar">
        <Input aria-label={t('pushOps.deviceSearch')} placeholder={t('pushOps.deviceSearch')} value={search} maxLength={200} onChange={e=>update('deviceQuery',e.target.value)}/>
        <Select aria-label={t('pushOps.provider')} value={provider} onChange={e=>update('deviceProvider',e.target.value)}><option value="">{t('pushOps.allProviders')}</option>{PROVIDERS.map(p=><option key={p} value={p}>{providerLabel(p)}</option>)}</Select>
        <Select
          aria-label={t('pushOps.deviceScope')}
          value={scope}
          onChange={(e) => update('scope', e.target.value)}
        >
          <option value="live">{t('pushOps.liveDevices')}</option>
          <option value="all">{t('pushOps.allDevices')}</option>
        </Select>
      </div>
      <ReadState loading={list.loading} error={list.error} />
      {list.data && (
        <>
          <DataTable
            appearance="design"
            rows={list.data.devices}
            columns={[
              {
                key: 'device',
                label: t('push.device'),
                render: (d) => (
                  <Action className="text-link" onClick={() => setSelected(d)}>
                    ···{d.tokenTail || d.id.slice(0, 8)}
                  </Action>
                ),
              },
              {
                key: 'provider',
                label: t('pushOps.provider'),
                render: (d) => providerLabel(d.provider),
              },
              {
                key: 'status',
                label: t('pushOps.status'),
                render: (d) => (
                  <Badge tone={d.revokedAt ? 'warning' : 'success'}>
                    {t(d.revokedAt ? 'pushOps.revoked' : 'pushOps.live')}
                  </Badge>
                ),
              },
              {
                key: 'user',
                label: t('pushOps.addressable'),
                render: (d) => t(d.addressable ? 'pushOps.yes' : 'pushOps.no'),
              },
              {
                key: 'seen',
                label: t('pushOps.lastSeen'),
                render: (d) => <TimeAgo iso={d.lastSeenAt} />,
              },
              {
                key: 'open',
                label: '',
                render: (d) => (
                  <Button small onClick={() => setSelected(d)}>
                    {t('pushOps.details')}
                  </Button>
                ),
              },
            ]}
          />
          {!list.data.devices.length && (
            <p className="py-6 muted">{t('push.devicesEmpty')}</p>
          )}
          <Pager
            offset={offset}
            total={list.data.total}
            move={(n) => update('deviceOffset', String(n))}
          />
        </>
      )}
      {selected && (
        <DeviceDialog
          project={project}
          device={selected}
          onClose={() => setSelected(null)}
          saved={() => {
            setSelected(null);
            list.reload();
          }}
        />
      )}
    </Surface>
  );
}
function DeviceDialog({
  project,
  device: d,
  onClose,
  saved,
}: {
  project: string;
  device: PushDevice;
  onClose: () => void;
  saved: () => void;
}) {
  const t = useT(),
    [confirm, setConfirm] = useState(false),
    command = usePushCommand(project, saved);
  return (
    <Dialog
      open
      title={t('push.deviceFacts')}
      closeLabel={t('common.close')}
      onClose={() => {
        if (!command.locked) onClose();
      }}
      footer={
        !d.revokedAt && (
          <Button
            variant="danger"
            busy={command.busy}
            onClick={() =>
              command.uncertain
                ? void command.run()
                : confirm
                  ? void command.run({
                      type: 'revokeDevice',
                      id: d.id,
                      expectedVersion: d.version,
                    })
                  : setConfirm(true)
            }
          >
            {command.uncertain
              ? t('common.retry')
              : t(confirm ? 'pushOps.confirmRevoke' : 'pushOps.revoke')}
          </Button>
        )
      }
    >
      <div className="push-stack">
        <code className="break-all">{d.id}</code>
        <p>
          {providerLabel(d.provider)} · {d.env ?? '—'}
        </p>
        <p>
          {t('pushOps.lastSeen')}: <TimeAgo iso={d.lastSeenAt} />
        </p>
        <p>
          {t('pushOps.badStreak')}: {d.badStreak}
        </p>
        <pre className="push-json">
          {JSON.stringify({ metadata: d.metadata, traits: d.traits }, null, 2)}
        </pre>
        {confirm && <p role="alert">{t('pushOps.revokeHint')}</p>}
        <CommandError command={command} />
      </div>
    </Dialog>
  );
}
function Credentials({ project }: { project: string }) {
  const t = useT(),
    list = useScopedAsyncData(project, () => api.pushCredentials(project)),
    [adding, setAdding] = useState(false),
    [selected, setSelected] = useState<PushCredential | null>(null);
  return (
    <div className="push-stack">
      <Surface
        title={t('push.credentialsTitle')}
        action={
          <Button
            variant="primary"
            disabled={!list.data?.vaultConfigured}
            onClick={() => setAdding(true)}
          >
            <Plus size={15} />
            {t('push.credentialsAdd')}
          </Button>
        }
      >
        <p className="muted text-sm mb-5">{t('pushOps.stagedHint')}</p>
        <ReadState loading={list.loading} error={list.error} />
        {list.data && !list.data.vaultConfigured && (
          <p role="status">{t('pushOps.vaultMissing')}</p>
        )}
        {list.data && (
          <>
            <DataTable
              appearance="design"
              rows={list.data.credentials}
              columns={[
                {
                  key: 'label',
                  label: t('push.credentialLabel'),
                  render: (c) => (
                    <Action
                      className="text-link"
                      onClick={() => setSelected(c)}
                    >
                      {c.label || providerLabel(c.kind)}
                    </Action>
                  ),
                },
                {
                  key: 'provider',
                  label: t('pushOps.provider'),
                  render: (c) => providerLabel(c.kind),
                },
                {
                  key: 'status',
                  label: t('pushOps.status'),
                  render: (c) => (
                    <Badge tone={c.active ? 'success' : 'neutral'}>
                      {t(c.active ? 'pushOps.active' : 'pushOps.staged')}
                    </Badge>
                  ),
                },
                {
                  key: 'probe',
                  label: t('pushOps.verification'),
                  render: (c) => (
                    <Badge
                      tone={
                        c.last_validate_status === 'ok'
                          ? 'success'
                          : c.last_validate_status === 'rejected'
                            ? 'danger'
                            : 'neutral'
                      }
                    >
                      {c.last_validate_status
                        ? t(
                            `pushOps.verdict.${c.last_validate_status}` as MessageKey,
                          )
                        : t('pushOps.unverified')}
                    </Badge>
                  ),
                },
                {
                  key: 'open',
                  label: '',
                  render: (c) => (
                    <Button small onClick={() => setSelected(c)}>
                      {t('pushOps.details')}
                    </Button>
                  ),
                },
              ]}
            />
            {!list.data.credentials.length && (
              <p className="py-6 muted">{t('push.credentialsEmpty')}</p>
            )}
          </>
        )}
      </Surface>
      {adding && (
        <CredentialEditor
          project={project}
          onClose={() => setAdding(false)}
          saved={() => {
            setAdding(false);
            list.reload();
          }}
        />
      )}
      {selected && (
        <CredentialDialog
          project={project}
          credential={selected}
          onClose={() => setSelected(null)}
          saved={() => {
            setSelected(null);
            list.reload();
          }}
        />
      )}
    </div>
  );
}
function CredentialEditor({
  project,
  onClose,
  saved,
}: {
  project: string;
  onClose: () => void;
  saved: () => void;
}) {
  const t = useT(),
    sharedT = useSharedT(),
    locale = useLocale(),
    [fileEpoch, setFileEpoch] = useState(0),
    [provider, setProvider] = useState<Provider>('apns'),
    [fields, setFields] = useState<Record<string, string>>({
      production: 'production',
    }),
    [label, setLabel] = useState(''),
    [fileError, setFileError] = useState<string | null>(null),
    [reading, setReading] = useState(false),
    command = usePushCommand(project, saved);
  const recognised = recognise(fields.secretKey ?? ''),
    locked = command.locked || reading;
  const change = (key: string, value: string) =>
    setFields((old) => ({ ...old, [key]: value }));
  async function file(value: File | undefined) {
    if (!value) return;
    setFileError(null);
    if (value.size > 65536) {
      setFileError(t('pushOps.fileTooLarge'));
      return;
    }
    setReading(true);
    try {
      const content = await value.text(),
        recognised = recognise(content, value.name);
      setFields((old) => ({
        ...old,
        secretKey: content,
        ...(recognised.keyId ? { keyId: recognised.keyId } : {}),
        ...(recognised.projectId ? { fcmProjectId: recognised.projectId } : {}),
      }));
    } catch (failure) {
      setFileError(formatApiError(failure));
    } finally {
      setReading(false);
    }
  }
  function submit() {
    const { secretKey = '', ...rest } = fields;
    const config: Record<string, unknown> = {};
    for (const field of PROVIDER_FIELDS[provider])
      if (field.key !== 'secretKey' && rest[field.key])
        config[field.key] =
          field.key === 'production'
            ? rest[field.key] === 'production'
            : rest[field.key];
    if (provider === 'fcm' && recognised.projectId)
      config.fcmProjectId = recognised.projectId;
    void command.run({
      type: 'createCredential',
      provider,
      config,
      secret: secretKey,
      label,
    });
  }
  return (
    <Dialog
      open
      title={t('push.credentialsAdd')}
      closeLabel={t('common.close')}
      onClose={() => {
        if (!locked) onClose();
      }}
      footer={
        <>
          <Button disabled={locked} onClick={onClose}>
            {t('team.cancel')}
          </Button>
          <Button
            variant="primary"
            busy={command.busy}
            type="submit"
            form="push-credential-editor"
            disabled={reading}
          >
            {command.uncertain ? t('common.retry') : t('pushOps.saveStaged')}
          </Button>
        </>
      }
    >
      <form
        id="push-credential-editor"
        className="form-stack"
        onSubmit={(e) => {
          e.preventDefault();
          if (command.uncertain) void command.run();
          else submit();
        }}
      >
        <fieldset className="form-stack min-w-0" disabled={locked}>
          <Field label={t('pushOps.provider')}>
            <Select
              value={provider}
              onChange={(e) => {
                setProvider(e.target.value as Provider);
                setFields({ production: 'production' });
                setFileEpoch((n) => n + 1);
              }}
            >
              {PROVIDER_SPECS.map((p) => (
                <option key={p.provider} value={p.provider}>
                  {providerLabel(p.provider)}
                </option>
              ))}
            </Select>
          </Field>
          <Field label={t('push.credentialLabel')}>
            <Input
              maxLength={100}
              value={label}
              onChange={(e) => setLabel(e.target.value)}
            />
          </Field>
          <div className="file-field">
            <p className="field-label">{t('pushOps.uploadCredential')}</p>
            <FileInput
              key={fileEpoch}
              t={sharedT}
              number={(value, digits = 0) =>
                value.toLocaleString(locale, { maximumFractionDigits: digits })
              }
              accept=".p8,.pem,.json"
              multiple={false}
              maxFiles={1}
              maxFileSize={65536}
              preview={false}
              disabled={locked}
              onChange={(files) => {
                if (files[0]) void file(files[0]);
                else change('secretKey', '');
              }}
            />
          </div>
          {PROVIDER_FIELDS[provider].map((f) => (
            <Field key={f.key} label={t(f.label)} hint={t(f.hint)}>
              {f.options ? (
                <Select
                  value={fields[f.key] ?? f.options[0].value}
                  onChange={(e) => change(f.key, e.target.value)}
                >
                  {f.options.map((o) => (
                    <option key={o.value} value={o.value}>
                      {t(o.label)}
                    </option>
                  ))}
                </Select>
              ) : f.multiline ? (
                <Textarea
                  required={f.required}
                  maxLength={65536}
                  spellCheck={false}
                  autoComplete="off"
                  value={fields[f.key] ?? ''}
                  onChange={(e) => change(f.key, e.target.value)}
                />
              ) : (
                <Input
                  required={f.required}
                  value={fields[f.key] ?? ''}
                  onChange={(e) => change(f.key, e.target.value)}
                />
              )}
            </Field>
          ))}
        </fieldset>
        {recognised.issue && (
          <p role="status">
            {t(`push.secretIssue.${recognised.issue}` as MessageKey)}
          </p>
        )}
        {fileError && <ErrorBanner>{fileError}</ErrorBanner>}
        <CommandError command={command} />
        <ProviderGuide provider={provider} />
      </form>
    </Dialog>
  );
}
function CredentialDialog({
  project,
  credential: c,
  onClose,
  saved,
}: {
  project: string;
  credential: PushCredential;
  onClose: () => void;
  saved: () => void;
}) {
  const t = useT(),
    command = usePushCommand(project, saved),
    [force, setForce] = useState(false),
    [deleting, setDeleting] = useState(false),
    [probing, setProbing] = useState(false),
    [probeError, setProbeError] = useState<string | null>(null);
  const locked = command.locked || probing;
  async function probe() {
    setProbing(true);
    setProbeError(null);
    try {
      await api.probePushCredential(project, c.id);
      saved();
    } catch (failure) {
      setProbeError(formatApiError(failure));
    } finally {
      setProbing(false);
    }
  }
  return (
    <Dialog
      open
      title={c.label || providerLabel(c.kind)}
      closeLabel={t('common.close')}
      onClose={() => {
        if (!locked) onClose();
      }}
      footer={
        <>
          <Button
            busy={probing}
            disabled={command.locked}
            onClick={() => void probe()}
          >
            {t('pushOps.probe')}
          </Button>
          <Button
            variant="danger"
            disabled={locked}
            onClick={() => setDeleting(true)}
          >
            {t('common.delete')}
          </Button>
          {!c.active && !command.uncertain && (
            <Button
              variant="primary"
              busy={command.busy}
              disabled={probing || (!force && c.last_validate_status !== 'ok')}
              onClick={() =>
                void command.run({
                  type: 'activateCredential',
                  id: c.id,
                  expectedVersion: c.version,
                  force,
                })
              }
            >
              {command.uncertain ? t('common.retry') : t('pushOps.activate')}
            </Button>
          )}
          {command.uncertain && (
            <Button busy={command.busy} onClick={() => void command.run()}>
              {t('common.retry')}
            </Button>
          )}
        </>
      }
    >
      <div className="push-stack">
        <Badge tone={c.active ? 'success' : 'neutral'}>
          {t(c.active ? 'pushOps.active' : 'pushOps.staged')}
        </Badge>
        <code className="break-all">{c.id}</code>
        <p>{t(c.encrypted ? 'pushOps.encrypted' : 'pushOps.legacy')}</p>
        <pre className="push-json">{JSON.stringify(c.config, null, 2)}</pre>
        <p>
          {t('pushOps.verification')}:{' '}
          {c.last_validate_status
            ? t(`pushOps.verdict.${c.last_validate_status}` as MessageKey)
            : t('pushOps.unverified')}
        </p>
        {c.last_validated_at && <TimeAgo iso={c.last_validated_at} />}{' '}
        {c.last_validate_detail && <code>{c.last_validate_detail}</code>}
        {c.problem && (
          <p>
            {t(`push.credError.${c.problem.code}` as MessageKey).replace(
              '{field}',
              c.problem.field ?? '',
            )}
          </p>
        )}
        {!c.active && c.last_validate_status !== 'ok' && (
          <Checkbox checked={force} disabled={locked} onChange={nextChecked => setForce(nextChecked)} label={t('pushOps.forceAck')}/>
        )}
        {deleting && (
          <div role="alert">
            <p>{t('pushOps.deleteHint')}</p>
            <Button
              variant="danger"
              busy={command.busy}
              disabled={command.uncertain}
              onClick={() =>
                void command.run({
                  type: 'deleteCredential',
                  id: c.id,
                  expectedVersion: c.version,
                })
              }
            >
              {t('pushOps.confirmDelete')}
            </Button>
          </div>
        )}
        {probeError && (
          <ErrorBanner reason={probeError}>{t('team.failed')}</ErrorBanner>
        )}
        <CommandError command={command} />
      </div>
    </Dialog>
  );
}
function Audience({ project }: { project: string }) {
  const t = useT(),
    [params] = useSearchParams(),
    [join, setJoin] = useState<'all' | 'any'>('all'),
    [rows, setRows] = useState<Condition[]>(() =>
      params.get('issue')
        ? [{ key: '', op: 'is', source: 'issue', value: params.get('issue')! }]
        : [blank()],
    ),
    [raw, setRaw] = useState<string | null>(null);
  const [preview, setPreview] = useState<Awaited<
      ReturnType<typeof api.previewAudience>
    > | null>(null),
    [title, setTitle] = useState(''),
    [body, setBody] = useState(''),
    [busy, setBusy] = useState(false),
    [uncertain, setUncertain] = useState(false),
    [error, setError] = useState<string | null>(null),
    [review, setReview] = useState(false),
    [sent, setSent] = useState<number | null>(null);
  const pending = useRef<Parameters<typeof api.sendToAudience> | null>(null),
    lock = useRef(false),
    sequence = useRef(0);
  let request: AudienceRequest | null = null;
  try {
    request =
      raw === null
        ? toRequest(join, rows)
        : { audience: JSON.parse(raw) as unknown };
  } catch {
    /* The editor displays invalid JSON below. */
  }
  const locked = busy || uncertain,
    edit = (fn: () => void) => {
      sequence.current++;
      fn();
      setPreview(null);
      setSent(null);
      setError(null);
    };
  async function count() {
    if (!request || lock.current) return;
    lock.current = true;
    setBusy(true);
    setError(null);
    const version = sequence.current;
    try {
      const result = await api.previewAudience(project, request);
      if (sequence.current === version) setPreview(result);
    } catch (failure) {
      setPreview(null);
      setError(formatApiError(failure));
    } finally {
      lock.current = false;
      setBusy(false);
    }
  }
  async function send() {
    if (lock.current) return;
    if (!pending.current) {
      if (!request || !preview) return;
      pending.current = [
        project,
        structuredClone(request),
        title,
        body,
        preview.matched,
        commandId(),
        preview.previewId,
      ];
    }
    lock.current = true;
    setBusy(true);
    setError(null);
    try {
      const result = await api.sendToAudience(...pending.current);
      pending.current = null;
      setUncertain(false);
      setSent(result.queued);
      setReview(false);
      setPreview(null);
    } catch (failure) {
      if (
        failure instanceof ApiError &&
        failure.status >= 400 &&
        failure.status < 500
      ) {
        pending.current = null;
        setUncertain(false);
        setPreview(null);
        setReview(false);
      } else setUncertain(true);
      setError(formatApiError(failure));
    } finally {
      lock.current = false;
      setBusy(false);
    }
  }
  return (
    <div className="push-stack">
      <Surface
        title={t('push.audienceTitle')}
        action={
          <Button
            small
            disabled={locked}
            onClick={() =>
              edit(() =>
                setRaw(
                  raw === null
                    ? JSON.stringify(request?.audience ?? {}, null, 2)
                    : null,
                ),
              )
            }
          >
            {t(
              raw === null ? 'push.audienceEditJson' : 'push.audienceEditRows',
            )}
          </Button>
        }
      >
        <fieldset disabled={locked} className="form-stack min-w-0">
          {raw !== null ? (
            <Field
              label={t('push.audienceEditJson')}
              error={!request ? t('push.audienceBadJson') : undefined}
            >
              <Textarea
                rows={8}
                value={raw}
                onChange={(e) => edit(() => setRaw(e.target.value))}
              />
            </Field>
          ) : (
            <>
              <Select
                aria-label={t('pushOps.join')}
                value={join}
                onChange={(e) =>
                  edit(() => setJoin(e.target.value as 'all' | 'any'))
                }
              >
                <option value="all">{t('push.joinAll')}</option>
                <option value="any">{t('push.joinAny')}</option>
              </Select>
              {rows.map((r, i) => (
                <div className="push-condition" key={i}>
                  <Select
                    aria-label={t('pushOps.source')}
                    value={r.source}
                    onChange={(e) =>
                      edit(() =>
                        setRows((old) =>
                          old.map((x, j) =>
                            i === j
                              ? { ...x, source: e.target.value as Source }
                              : x,
                          ),
                        ),
                      )
                    }
                  >
                    {Object.entries({
                      trait: 'push.sourceTrait',
                      device: 'push.sourceDevice',
                      user: 'push.sourceUser',
                      issue: 'push.sourceIssue',
                    } as const).map(([source, label]) => (
                      <option key={source} value={source}>
                        {t(label)}
                      </option>
                    ))}
                  </Select>
                  {!valueOnly(r) && (
                    <Input
                      aria-label={t('pushOps.key')}
                      value={r.key}
                      onChange={(e) =>
                        edit(() =>
                          setRows((old) =>
                            old.map((x, j) =>
                              i === j ? { ...x, key: e.target.value } : x,
                            ),
                          ),
                        )
                      }
                    />
                  )}
                  <Select
                    aria-label={t('pushOps.operator')}
                    value={r.op}
                    onChange={(e) =>
                      edit(() =>
                        setRows((old) =>
                          old.map((x, j) =>
                            i === j ? { ...x, op: e.target.value as Op } : x,
                          ),
                        ),
                      )
                    }
                  >
                    {OPS.map((o) => (
                      <option key={o.value} value={o.value}>
                        {t(o.label)}
                      </option>
                    ))}
                  </Select>
                  {r.op !== 'exists' && (
                    <Input
                      aria-label={t('pushOps.value')}
                      value={r.value}
                      onChange={(e) =>
                        edit(() =>
                          setRows((old) =>
                            old.map((x, j) =>
                              i === j ? { ...x, value: e.target.value } : x,
                            ),
                          ),
                        )
                      }
                    />
                  )}
                  <Button
                    small
                    disabled={rows.length === 1}
                    onClick={() =>
                      edit(() =>
                        setRows((old) => old.filter((_, j) => i !== j)),
                      )
                    }
                  >
                    {t('common.delete')}
                  </Button>
                </div>
              ))}
              <Button
                onClick={() => edit(() => setRows((old) => [...old, blank()]))}
                disabled={rows.length >= 20}
              >
                {t('pushOps.addCondition')}
              </Button>
            </>
          )}
          <Button
            variant="primary"
            busy={busy}
            disabled={!request}
            onClick={() => void count()}
          >
            {t('pushOps.preview')}
          </Button>
        </fieldset>
        {preview && (
          <div className="push-stack mt-5">
            <p>
              <strong>{preview.matched}</strong> {t('pushOps.matched')}
            </p>
            <p className="muted text-sm">
              {t('pushOps.previewExpires')}:{' '}
              <time dateTime={preview.expiresAt}>
                {new Date(preview.expiresAt).toLocaleTimeString()}
              </time>
            </p>
            <DataTable
              appearance="design"
              rows={preview.sample}
              columns={[
                {
                  key: 'id',
                  label: t('push.device'),
                  render: (d) => <code>{d.id.slice(0, 8)}</code>,
                },
                {
                  key: 'provider',
                  label: t('pushOps.provider'),
                  render: (d) => providerLabel(d.provider),
                },
                {
                  key: 'traits',
                  label: t('pushOps.traits'),
                  render: (d) => (
                    <code className="break-all">
                      {JSON.stringify(d.traits)}
                    </code>
                  ),
                },
              ]}
            />
          </div>
        )}
      </Surface>
      <Surface title={t('push.audienceMessage')}>
        <fieldset className="form-stack min-w-0" disabled={locked}>
          <Field label={t('push.testSubject')}>
            <Input
              required
              maxLength={120}
              value={title}
              onChange={(e) => setTitle(e.target.value)}
            />
          </Field>
          <Field label={t('push.testBody')}>
            <Textarea
              maxLength={3000}
              value={body}
              onChange={(e) => setBody(e.target.value)}
            />
          </Field>
          <Button
            variant="primary"
            disabled={!preview?.matched || !title.trim()}
            onClick={() => setReview(true)}
          >
            {t('pushOps.review')}
          </Button>
        </fieldset>
        {sent !== null && (
          <p role="status">
            {t('push.audienceQueued').replace('{n}', String(sent))}
          </p>
        )}
        {error && !review && (
          <ErrorBanner reason={error}>{t('team.failed')}</ErrorBanner>
        )}
      </Surface>
      {review && (
        <Dialog
          open
          title={t('pushOps.review')}
          closeLabel={t('common.close')}
          onClose={() => {
            if (!locked) setReview(false);
          }}
          footer={
            <>
              <Button disabled={locked} onClick={() => setReview(false)}>
                {t('team.cancel')}
              </Button>
              <Button variant="primary" busy={busy} onClick={() => void send()}>
                {uncertain ? t('common.retry') : t('pushOps.confirmSend')}
              </Button>
            </>
          }
        >
          <div className="push-stack">
            <p>
              {t('push.confirmSend').replace(
                '{n}',
                String(preview?.matched ?? 0),
              )}
            </p>
            <strong>{title}</strong>
            <p className="whitespace-pre-wrap break-words">{body}</p>
            <pre className="push-json">{JSON.stringify(request, null, 2)}</pre>
            {error && (
              <ErrorBanner reason={error}>{t('team.failed')}</ErrorBanner>
            )}
            {uncertain && <p>{t('team.uncertain')}</p>}
          </div>
        </Dialog>
      )}
    </div>
  );
}

function Readiness({
  project,
  onGo,
}: {
  project: string;
  onGo: (tab: string) => void;
}) {
  const t = useT(),
    read = useScopedAsyncData(project, () => api.pushReadiness(project));
  const destination: Record<string, string> = {
    'no-credential': 'credentials',
    'credential-unused': 'credentials',
    'mass-quarantine': 'devices',
    'all-failing': 'delivery',
    'queue-stalled': 'delivery',
  };
  const fill = (text: string, data: Record<string, unknown>) =>
    text.replace(/\{(\w+)\}/g, (all, key: string) =>
      data[key] == null ? all : String(data[key]),
    );
  return (
    <Surface title={t('push.readinessTitle')}>
      <ReadState loading={read.loading} error={read.error} />
      {read.data && (
        <div className="push-stack">
          <p className="muted text-sm">{t('pushOps.setupOnly')}</p>
          {read.data.checks.length ? (
            read.data.checks.map((c) => (
              <div key={c.id}>
                <Badge
                  tone={
                    c.level === 'blocked'
                      ? 'danger'
                      : c.level === 'warn'
                        ? 'warning'
                        : 'info'
                  }
                >
                  {t(`push.level.${c.level}`)}
                </Badge>
                <p className="mt-2">
                  {fill(t(`push.check.${c.id}` as MessageKey), c.data)}
                </p>
                <p className="muted text-sm mt-1">
                  {fill(t(`push.fix.${c.id}` as MessageKey), c.data)}
                </p>
                {destination[c.id] && (
                  <Button
                    small
                    className="mt-2"
                    onClick={() => onGo(destination[c.id])}
                  >
                    {t(`push.section.${destination[c.id]}` as MessageKey)}
                  </Button>
                )}
              </div>
            ))
          ) : (
            <p>{t('pushOps.setupReady')}</p>
          )}
        </div>
      )}
    </Surface>
  );
}
