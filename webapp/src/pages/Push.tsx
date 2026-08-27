// Push — its own module, not a settings tab.
//
// It was one: four panels stacked inside `Settings?tab=push`, which
// put "did last night's alert reach anyone" behind the same door as
// "change my password". They are not the same job. Settings is
// somewhere you go once to configure a thing; this is somewhere you
// come back to when something did not arrive, and it has its own
// nouns — devices, credentials, sends, receipts.
//
// Four sections, because the questions are four:
//   delivery    — did it go out, and if not, why
//   audience    — who the next one is for, and how many that is
//   devices     — who can be reached, and what did they tell us
//   credentials — what we send with
//
// The section lives in the URL. A screen nobody can link to is a
// screen nobody can point a colleague at, and one no screenshot
// sweep can reach.
//
// The history is worth keeping: push was a complete backend for a
// year with no dashboard at all, which is why it had no users — the
// only way to give Sentori an APNs key was an admin API nobody could
// see. It got a settings tab, then a first real integrator, who
// found that the credential form had no field for the secret and
// that the test-send endpoint had never been wired to anything. Both
// are here now.

import {
  Check,
  CircleCheck,
  CircleX,
  Copy,
  ExternalLink,
  Info,
  TriangleAlert,
  Upload,
} from 'lucide-react';
import { Fragment, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';

import { useShell } from '../App';
import {
  Button,
  buttonClass,
  DataTable,
  ErrorBanner,
  Field,
  Input,
  PageShell,
  Panel,
  PanelEmpty,
  Select,
  Textarea,
  clsx,
  formatRelative,
} from '../components/ui';
import { useT } from '../i18n';
import type { MessageKey } from '../i18n/en';
import { ApiError, api } from '../lib/api';
import type {
  AudienceRequest,
  AudienceSample,
  ProbeVerdict,
  PushCheck,
  PushCredential,
} from '../lib/api';
import type { Provider } from '../lib/push-credentials';
import { PROVIDER_SPECS, recognise, verdictTone } from '../lib/push-credentials';
import { highlightBlock } from '../lib/highlight';
import { useAsyncData } from '../lib/useAsyncData';
import {
  SEND_PATH,
  SNIPPET_LANGS,
  type SnippetLang,
  countSnippet,
  pollSnippet,
  snippet,
} from '../lib/push-snippets';

type Section = 'audience' | 'credentials' | 'delivery' | 'devices' | 'integrate';
const SECTIONS: Section[] = ['delivery', 'audience', 'devices', 'credentials', 'integrate'];

export default function PushPage() {
  const t = useT();
  const { activeProject } = useShell();
  const projectId = activeProject?.id ?? '';

  const [params, setParams] = useSearchParams();
  const asked = params.get('tab') as null | Section;
  const section: Section = asked && SECTIONS.includes(asked) ? asked : 'delivery';
  const setSection = (x: Section) => setParams({ tab: x }, { replace: true });

  return (
    <PageShell
      title={t('nav.push')}
      toolbar={
        <div className="flex items-center gap-1">
          {SECTIONS.map((x) => (
            <button
              key={x}
              type="button"
              onClick={() => setSection(x)}
              className={clsx(
                'rounded px-2 py-1 text-xs transition-colors',
                section === x
                  ? 'bg-raised font-medium text-fg'
                  : 'text-fg-subtle hover:text-fg-muted',
              )}
            >
              {t(`push.section.${x}`)}
            </button>
          ))}
        </div>
      }
    >
      {!projectId ? (
        <PanelEmpty>{t('instruments.noProject')}</PanelEmpty>
      ) : section === 'delivery' ? (
        <DeliverySection projectId={projectId} onGo={setSection} />
      ) : section === 'audience' ? (
        <AudienceSection projectId={projectId} />
      ) : section === 'integrate' ? (
        <IntegrateSection />
      ) : section === 'devices' ? (
        <DevicesSection projectId={projectId} />
      ) : (
        <CredentialsSection projectId={projectId} />
      )}
    </PageShell>
  );
}

/// One reading: the number, then what it is.
///
/// Value over label, tabular, so a column of them lines up and the
/// eye can tell a number from the words either side of it. Tone is
/// semantic and sparing — a zero failure count is not green news, it
/// is the absence of news.
function Stat({
  label,
  tone,
  value,
}: {
  label: string;
  tone?: 'bad' | 'ok' | 'warn';
  value: number | string;
}) {
  return (
    <div className="flex flex-col gap-0.5">
      <span
        className={clsx(
          'font-mono text-[17px] leading-none tabular-nums',
          tone === 'ok' && 'text-ok',
          tone === 'bad' && 'text-kind-error',
          tone === 'warn' && 'text-kind-warn',
        )}
      >
        {value}
      </span>
      <span className="text-[11px] uppercase tracking-wide text-fg-subtle">{label}</span>
    </div>
  );
}

/// Where you are in a list, and how to move.
///
/// Both tables capped at a hundred rows and said nothing about it, so
/// a project with four hundred devices showed a hundred and looked
/// complete. The count is the part that matters: a table headed
/// "50" when there are four hundred is not a smaller truth, it is a
/// different one.
function Pager({
  offset,
  onOffset,
  page,
  shown,
  total,
}: {
  offset: number;
  onOffset: (n: number) => void;
  page: number;
  shown: number;
  total: number;
}) {
  const t = useT();
  if (total <= page && offset === 0) return null;
  const from = total === 0 ? 0 : offset + 1;
  const to = offset + shown;
  return (
    <div className="flex items-center gap-2 border-t border-border/60 px-3.5 py-2">
      <span className="text-xs tabular-nums text-fg-subtle">
        {t('push.range', { from: String(from), to: String(to), total: String(total) })}
      </span>
      <div className="ml-auto flex items-center gap-1">
        <Button
          size="sm"
          disabled={offset === 0}
          onClick={() => onOffset(Math.max(0, offset - page))}
        >
          {t('push.prev')}
        </Button>
        <Button size="sm" disabled={to >= total} onClick={() => onOffset(offset + page)}>
          {t('push.next')}
        </Button>
      </div>
    </div>
  );
}

// ── delivery ────────────────────────────────────────────────────────

const PAGE = 50;

function DeliverySection({
  projectId,
  onGo,
}: {
  onGo: (s: Section) => void;
  projectId: string;
}) {
  const t = useT();
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState('');
  const [offset, setOffset] = useState(0);

  const health = useAsyncData(() => api.pushHealth(projectId), [projectId]);
  // Filtered and paged by the server. Filtering a page in the browser
  // filters the page, not the data: with fifty of four hundred rows
  // in hand, "failed" showed the failures among the fifty.
  const sends = useAsyncData(
    () => api.pushSends(projectId, PAGE, status, offset),
    [projectId, status, offset],
  );
  const devices = useAsyncData(() => api.pushDevices(projectId), [projectId]);

  const h = health.data;
  const rows = sends.data?.sends ?? [];
  const total = sends.data?.total ?? rows.length;

  const reload = () => {
    health.reload();
    sends.reload();
  };
  const refilter = (next: string) => {
    setStatus(next);
    // Page one of the new question, not page four of the old one.
    setOffset(0);
  };

  return (
    <div className="space-y-4">
      {/* First, because it answers the question someone came here
          with. The numbers below say what happened; this says whether
          anything could have. */}
      <ReadinessPanel projectId={projectId} onGo={onGo} />
      <Panel title={t('push.deliveryTitle')}>
        {!h || (h.sent24h === 0 && h.failed24h === 0 && h.queued === 0) ? (
          <PanelEmpty>{t('push.deliveryEmpty')}</PanelEmpty>
        ) : (
          <div className="space-y-2.5 p-3.5">
            {/* Six readings on one rail, value over label. They used to
                run together inline — `412 24 小时送达 7 失败` — where
                the eye cannot tell a number from the words either side
                of it. */}
            <div className="flex flex-wrap items-start gap-x-8 gap-y-3">
              <Stat value={h.sent24h} label={t('push.sent24h')} tone={h.sent24h > 0 ? 'ok' : undefined} />
              <Stat
                value={h.failed24h}
                label={t('push.failed24h')}
                tone={h.failed24h > 0 ? 'bad' : undefined}
              />
              <Stat value={h.queued} label={t('push.queued')} />
              <div className="ml-auto flex items-start gap-x-8">
                <Stat value={h.liveTokens} label={t('push.statLive')} />
                <Stat value={h.identifiedTokens} label={t('push.statIdentified')} />
                <Stat
                  value={h.quarantinedTokens}
                  label={t('push.statQuarantined')}
                  tone={h.quarantinedTokens > 0 ? 'warn' : undefined}
                />
              </div>
            </div>
            {/* A count is an alarm; a reason is a fix. */}
            {h.reasons.length > 0 && (
              <div className="flex flex-wrap gap-x-4 gap-y-1 pt-1 text-xs text-fg-muted">
                {h.reasons.map((r) => (
                  <span key={r.reason} className="font-mono">
                    {r.reason} <span className="text-fg-subtle">×{r.count}</span>
                  </span>
                ))}
              </div>
            )}
          </div>
        )}
      </Panel>

      <TestSend projectId={projectId} devices={devices.data?.devices ?? []} onSent={reload} />

      <Panel
        title={`${t('push.sendsTitle')} (${total})`}
        action={
          <div className="flex items-center gap-2">
            <Select
              value={status}
              onChange={(e) => refilter(e.target.value)}
              aria-label={t('push.filterStatus')}
            >
              <option value="">{t('push.statusAny')}</option>
              <option value="sent">sent</option>
              <option value="failed">failed</option>
              <option value="queued">queued</option>
            </Select>
            {rows.some((r) => r.status === 'failed') && (
              <Button
                size="sm"
                disabled={busy}
                onClick={() => {
                  setBusy(true);
                  void api
                    .retryAllFailedPushSends(projectId)
                    .then(reload)
                    .finally(() => setBusy(false));
                }}
              >
                {t('push.retryAll')}
              </Button>
            )}
          </div>
        }
      >
        <DataTable
          rows={rows}
          rowKey={(r) => r.id}
          empty={t('push.sendsEmpty')}
          columns={[
            {
              // Which notification this row is. Without it a failed
              // row is an outcome with nothing attached to it.
              key: 'message',
              label: t('push.message'),
              render: (r) => (
                <span className="truncate text-xs">
                  {typeof r.payload?.title === 'string' && r.payload.title.length > 0 ? (
                    r.payload.title
                  ) : (
                    <span className="text-fg-subtle">{t('push.messageNone')}</span>
                  )}
                </span>
              ),
            },
            {
              key: 'provider',
              label: t('push.provider'),
              width: '110px',
              render: (r) => <span className="font-mono text-xs">{r.provider}</span>,
            },
            {
              key: 'status',
              label: t('instruments.colStatus'),
              width: '110px',
              render: (r) => (
                <span
                  className={clsx(
                    'font-mono text-xs',
                    r.status === 'failed' && 'text-kind-error',
                    r.status === 'sent' && 'text-ok',
                  )}
                >
                  {r.status}
                </span>
              ),
            },
            {
              // The provider's own words. A `queued` row carries the
              // reason its last attempt failed, which is how a send
              // that is retrying is told apart from one that is
              // merely waiting — they used to look identical.
              key: 'error',
              label: t('push.reason'),
              render: (r) => (
                <span className="text-xs text-fg-muted">{r.error ?? r.provider_outcome ?? '—'}</span>
              ),
            },
            {
              key: 'created_at',
              label: t('settings.colWhen'),
              width: '110px',
              align: 'right',
              render: (r) => (
                <span className="text-xs tabular-nums text-fg-subtle">
                  {formatRelative(r.created_at)}
                </span>
              ),
            },
            {
              key: 'retry',
              label: '',
              width: '70px',
              align: 'right',
              render: (r) =>
                r.status === 'failed' ? (
                  <button
                    type="button"
                    className="text-xs text-fg-subtle hover:text-fg"
                    onClick={() => {
                      void api.retryPushSend(projectId, r.id).then(reload);
                    }}
                  >
                    {t('push.retry')}
                  </button>
                ) : null,
            },
          ]}
        />
        <Pager
          offset={offset}
          onOffset={setOffset}
          page={PAGE}
          shown={rows.length}
          total={total}
        />
      </Panel>
    </div>
  );
}

/// Send one, to a device you pick, and watch what comes back.
///
/// `api.pushTest` has existed the whole time, with a comment
/// describing the UI that would ask for a device first. Nothing ever
/// called it. So the only way to answer "does push work at all" was
/// to mint an api-scope token and curl `/v1/push/send` — which is
/// exactly what our first integrator did, from a terminal, to test
/// the product's own feature.
function TestSend({
  projectId,
  devices,
  onSent,
}: {
  devices: { addressable: boolean; env: null | string; id: string; provider: string; revokedAt: null | string; tokenTail: null | string }[];
  onSent: () => void;
  projectId: string;
}) {
  const t = useT();
  const reachable = devices.filter((d) => !d.revokedAt);
  const [deviceId, setDeviceId] = useState('');
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<null | { id: string; reason?: string; status: string }>(
    null,
  );
  const [error, setError] = useState<null | string>(null);

  const target = deviceId || reachable[0]?.id || '';

  return (
    <Panel title={t('push.testTitle')}>
      {reachable.length === 0 ? (
        <PanelEmpty>{t('push.testNoDevice')}</PanelEmpty>
      ) : (
        <>
          <div className="flex flex-wrap items-end gap-3 p-3.5">
            <Field label={t('push.device')}>
              <Select value={target} onChange={(e) => setDeviceId(e.target.value)}>
                {reachable.map((d) => (
                  <option key={d.id} value={d.id}>
                    {d.provider}
                    {d.env ? `/${d.env}` : ''} ···{d.tokenTail ?? ''}
                  </option>
                ))}
              </Select>
            </Field>
            <Field label={t('push.testSubject')} className="min-w-0 flex-1">
              <Input
                value={title}
                onChange={(e) => setTitle(e.target.value)}
                placeholder={t('push.testSubjectPlaceholder')}
              />
            </Field>
            <Field label={t('push.testBody')} className="min-w-0 flex-1">
              <Input
                value={body}
                onChange={(e) => setBody(e.target.value)}
                placeholder={t('push.testBodyPlaceholder')}
              />
            </Field>
            <Button
              variant="primary"
              size="sm"
              disabled={busy || title.trim().length === 0}
              onClick={() => {
                setBusy(true);
                setResult(null);
                setError(null);
                void api
                  .pushTest(projectId, target, title, body)
                  .then(async (r) => {
                    // The server answers with an id, not an outcome:
                    // the worker has not tried yet. Saying "sent"
                    // here would be the same lie the receipts used to
                    // tell — so it says queued, and then waits.
                    if (r.error || !r.sendId) {
                      setError(r.error ?? 'no send id');
                      return;
                    }
                    setResult({ id: r.sendId, status: 'queued' });
                    onSent();
                    // A test whose answer is "it is on its way" is
                    // not a test. The worker takes seconds; watch
                    // until it is done, then say what happened —
                    // including the provider's own words when it
                    // failed, which is the whole reason to press this
                    // button rather than send from a terminal.
                    const deadline = Date.now() + 60_000;
                    while (Date.now() < deadline) {
                      await new Promise((res) => setTimeout(res, 1500));
                      const page = await api.pushSends(projectId, 20);
                      const row = page.sends.find((x) => x.id === r.sendId);
                      if (!row) continue;
                      setResult({
                        id: r.sendId,
                        reason: row.error ?? row.provider_outcome ?? undefined,
                        status: row.status,
                      });
                      onSent();
                      if (row.status !== 'queued') return;
                    }
                  })
                  .catch((e: Error) => setError(e.message))
                  .finally(() => setBusy(false));
              }}
            >
              {t('push.testSend')}
            </Button>
          </div>
          {error && <ErrorBanner>{error}</ErrorBanner>}
          {result !== null && (
            <div className="border-t border-border/60 px-3.5 py-2 text-xs">
              <span
                className={clsx(
                  'font-mono',
                  result.status === 'failed' && 'text-kind-error',
                  result.status === 'sent' && 'text-ok',
                  result.status === 'queued' && 'text-fg-muted',
                )}
              >
                {result.status}
              </span>{' '}
              <span className="text-fg-muted">
                {result.status === 'queued' ? t('push.testWaiting') : ''}
              </span>
              {result.reason && (
                <span className="text-fg-muted"> — {result.reason}</span>
              )}
              <span className="ml-2 font-mono text-fg-subtle">{result.id}</span>
            </div>
          )}
        </>
      )}
    </Panel>
  );
}



// ── readiness ───────────────────────────────────────────────────────

/// What is set up, what is missing, and what to do about it.
///
/// Push has a lot of ways to be almost configured, and from the
/// console they all look the same: devices arrive, sends queue,
/// nothing lands. Every line here was somebody asking us why —
/// a project with three hundred FCM devices and no FCM credential, a
/// project whose devices never called `user()` so every send aimed at
/// a person reached nobody and reported success.
///
/// The server sends codes and numbers. The words are here, because
/// this is where they get translated.

/// Which grammar lights which snippet.
///
/// `node` is the odd one: the snippet is TypeScript, which is what a
/// Bun or Deno backend writes and what Node reads with types stripped.
const HLJS_LANG: Record<SnippetLang, string> = {
  cpp: 'cpp',
  csharp: 'csharp',
  go: 'go',
  java: 'java',
  node: 'typescript',
  python: 'python',
  rust: 'rust',
};

/// Where to go to fix it, when there is somewhere to go.
const FIX_SECTION: Record<string, Section> = {
  'all-failing': 'delivery',
  'credential-unused': 'credentials',
  'mass-quarantine': 'devices',
  'no-credential': 'credentials',
  'queue-stalled': 'delivery',
};

function ReadinessPanel({
  projectId,
  onGo,
}: {
  onGo: (s: Section) => void;
  projectId: string;
}) {
  const t = useT();
  const r = useAsyncData(() => api.pushReadiness(projectId), [projectId]);
  const checks = r.data?.checks ?? [];

  if (r.loading || r.error) return null;

  return (
    <Panel title={t('push.readinessTitle')}>
      {checks.length === 0 ? (
        <div className="flex items-center gap-2 px-3.5 py-3 text-sm">
          <CircleCheck aria-hidden className="size-3.5 shrink-0 text-ok" />
          <span className="text-fg-muted">
            {r.data && r.data.live > 0
              ? t('push.readyWithDevices').replace('{n}', String(r.data.live))
              : t('push.ready')}
          </span>
        </div>
      ) : (
        <ul className="flex flex-col divide-y divide-border/60">
          {checks.map((c) => {
            const where = FIX_SECTION[c.id];
            return (
              <li key={c.id} className="flex flex-wrap items-baseline gap-x-2 gap-y-1 px-3.5 py-2.5">
                <LevelIcon level={c.level} label={t(`push.level.${c.level}` as MessageKey)} />
                <span className="text-sm">{fill(t(`push.check.${c.id}` as MessageKey), c.data)}</span>
                <span className="w-full text-xs text-fg-muted">
                  {fill(t(`push.fix.${c.id}` as MessageKey), c.data)}
                  {where && (
                    <button
                      type="button"
                      className="ml-1.5 text-accent hover:underline"
                      onClick={() => onGo(where)}
                    >
                      {t(`push.section.${where}`)} →
                    </button>
                  )}
                </span>
              </li>
            );
          })}
        </ul>
      )}
    </Panel>
  );
}

/// The level, as a mark rather than a word.
///
/// Three of these stack up per project and the words for them were
/// wider than the facts they qualified. The name stays as the
/// accessible label — an icon-only severity is a severity a screen
/// reader cannot read.
function LevelIcon({ label, level }: { label: string; level: PushCheck['level'] }) {
  const Icon = level === 'blocked' ? CircleX : level === 'warn' ? TriangleAlert : Info;
  return (
    <Icon
      aria-label={label}
      role="img"
      className={clsx(
        'size-3.5 shrink-0 translate-y-0.5',
        level === 'blocked' && 'text-kind-error',
        level === 'warn' && 'text-kind-warn',
        level === 'info' && 'text-fg-subtle',
      )}
    />
  );
}

/// What is wrong with a credential, in the language the console is in.
///
/// The server sends a code and the field it is about. It used to send
/// English prose, which this printed under a Chinese label.
function credentialProblem(
  t: (k: MessageKey) => string,
  p: { code: string; field: null | string },
): string {
  const key = `push.credError.${p.code}` as MessageKey;
  const text = t(key);
  // A code this build does not know still has to say something. The
  // server and the console ship together, so this is the gap between
  // an upgraded server and a cached bundle.
  if (text === key) return p.field ?? p.code;
  return p.field ? text.replace('{field}', p.field) : text;
}

/// Put the server's numbers into the console's sentence.
function fill(text: string, data: Record<string, unknown>): string {
  return text.replace(/\{(\w+)\}/g, (whole, key: string) => {
    const v = data[key];
    return v === undefined || v === null ? whole : String(v);
  });
}

// ── audience ────────────────────────────────────────────────────────

/// Who a send is for.
///
/// The server takes three shapes — one user, a set of attributes, or
/// an expression — and compiles all three into the same query, so this
/// is one editor rather than three tabs. A row naming a user is the
/// first shape; several rows of equalities are the second; anything
/// else is the third.
///
/// Nothing here sends without a count first. The only other way to
/// find out what a condition matches is to send to it, and a
/// notification cannot be recalled.

type Source = 'device' | 'issue' | 'trait' | 'user';
type Op = 'exists' | 'gte' | 'in' | 'is' | 'isNot' | 'lte' | 'prefix' | 'versionGte' | 'versionLte';

type Condition = { key: string; op: Op; source: Source; value: string };

const OPS: { label: MessageKey; value: Op }[] = [
  { label: 'push.opIs', value: 'is' },
  { label: 'push.opIsNot', value: 'isNot' },
  { label: 'push.opIn', value: 'in' },
  { label: 'push.opPrefix', value: 'prefix' },
  { label: 'push.opExists', value: 'exists' },
  { label: 'push.opVersionGte', value: 'versionGte' },
  { label: 'push.opVersionLte', value: 'versionLte' },
  { label: 'push.opGte', value: 'gte' },
  { label: 'push.opLte', value: 'lte' },
];

const blank = (): Condition => ({ key: '', op: 'is', source: 'trait', value: '' });

/// A key for one send. `crypto.randomUUID` needs a secure context,
/// which the dashboard is; the fallback is for the one that is not.
function mintKey(): string {
  const c = globalThis.crypto as undefined | { randomUUID?: () => string };
  if (typeof c?.randomUUID === 'function') return c.randomUUID();
  return `k-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}

/// Sources that name a thing rather than an attribute of one, so the
/// row is one field wide instead of three.
const valueOnly = (r: Condition) => r.source === 'user' || r.source === 'issue';

/// A written value, as the type the server compares with.
///
/// `"true"` and `"42"` come out of a text field as strings, and a
/// trait stored as a boolean would then never match one. Version and
/// prefix comparisons are the exception — those are strings by
/// definition, and "4.20" must not become the number 4.2.
function typed(op: Op, raw: string): unknown {
  const s = raw.trim();
  if (op === 'prefix' || op === 'versionGte' || op === 'versionLte') return s;
  if (s === 'true') return true;
  if (s === 'false') return false;
  if (s === 'null') return null;
  if (s !== '' && !Number.isNaN(Number(s))) return Number(s);
  return s;
}

/// The rows as the server's shape, or null when a row is unfinished.
///
/// Null rather than a partial expression: a half-written condition
/// that quietly compiles matches something nobody asked for, and the
/// count next to it would look like an answer.
function toRequest(join: 'all' | 'any', rows: Condition[]): AudienceRequest | null {
  const usable = rows.filter((r) => (valueOnly(r) ? r.value.trim() : r.key.trim()));
  if (usable.length === 0) return null;

  const leaves = usable.map((r) => {
    if (r.source === 'user') return { user: r.value.trim() };
    // Everyone an issue happened to. The server joins it against the
    // same identity hash the events carry, so this is the condition
    // behind "tell the people who hit this that it is fixed".
    if (r.source === 'issue') return { issue: r.value.trim() };
    const where = r.source === 'trait' ? { trait: r.key.trim() } : { device: r.key.trim() };
    if (r.op === 'exists') return { ...where, exists: true };
    if (r.op === 'in') {
      return {
        ...where,
        in: r.value
          .split(',')
          .map((v) => v.trim())
          .filter((v) => v !== '')
          .map((v) => typed('is', v)),
      };
    }
    return { ...where, [r.op]: typed(r.op, r.value) };
  });

  return { audience: leaves.length === 1 && join === 'all' ? leaves[0] : { [join]: leaves } };
}

function AudienceSection({ projectId }: { projectId: string }) {
  const t = useT();
  const [params] = useSearchParams();
  const [join, setJoin] = useState<'all' | 'any'>('all');
  // Seeded from the URL when the issue page sent you here, so the
  // trip from "this is fixed" to "tell the people it happened to" is
  // a click rather than a copied uuid. Read once: it is a starting
  // value, not state the address bar keeps owning.
  const [rows, setRows] = useState<Condition[]>(() => {
    const issue = params.get('issue');
    return issue ? [{ key: '', op: 'is', source: 'issue', value: issue }] : [blank()];
  });
  const [raw, setRaw] = useState<null | string>(null);
  const [preview, setPreview] = useState<null | { matched: number; sample: AudienceSample[] }>(
    null,
  );
  // Minted with the count and sent with the send, so pressing the
  // button twice queues once. Counting again mints a new one, which is
  // the only way to deliberately send the same thing twice.
  const [sendKey, setSendKey] = useState('');
  const [title, setTitle] = useState('');
  const [body, setBody] = useState('');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<null | string>(null);
  const [sent, setSent] = useState<null | number>(null);

  // The expression as the server will read it: the rows, unless
  // someone has opened the JSON view, in which case what they typed
  // wins — the escape hatch is only an escape if it is authoritative.
  let request: AudienceRequest | null = null;
  let rawError: null | string = null;
  if (raw === null) {
    request = toRequest(join, rows);
  } else {
    try {
      request = { audience: JSON.parse(raw) as unknown };
    } catch {
      rawError = t('push.audienceBadJson');
    }
  }

  // Any edit invalidates the count, and the count is what the send is
  // allowed to use. Leaving a stale number on screen next to a Send
  // button is how someone sends to an audience they last saw an hour
  // ago.
  const edit = (fn: () => void) => {
    fn();
    setPreview(null);
    setSent(null);
    setError(null);
  };

  const runPreview = () => {
    if (!request) return;
    setBusy(true);
    setError(null);
    void api
      .previewAudience(projectId, request)
      .then((p) => {
        setPreview(p);
        setSendKey(mintKey());
      })
      .catch((e: Error) => {
        setPreview(null);
        setError(e.message);
      })
      .finally(() => setBusy(false));
  };

  return (
    <div className="flex flex-col gap-3">
      <Panel
        title={t('push.audienceTitle')}
        action={
          <button
            type="button"
            className="text-xs text-fg-subtle hover:text-fg-muted"
            onClick={() =>
              edit(() => setRaw(raw === null ? JSON.stringify(request?.audience ?? {}, null, 2) : null))
            }
          >
            {raw === null ? t('push.audienceEditJson') : t('push.audienceEditRows')}
          </button>
        }
      >
        {raw !== null ? (
          <div className="p-3.5" data-audience-editor>
            <Textarea
              rows={10}
              value={raw}
              onChange={(e) => edit(() => setRaw(e.target.value))}
              className="font-mono text-xs"
            />
            <p className="mt-2 text-xs text-fg-subtle">{t('push.audienceJsonHint')}</p>
          </div>
        ) : (
          <div className="flex flex-col gap-2 p-3.5" data-audience-editor>
            <div className="flex items-center gap-2 text-xs text-fg-muted">
              <Select
                value={join}
                onChange={(e) => edit(() => setJoin(e.target.value as 'all' | 'any'))}
              >
                <option value="all">{t('push.joinAll')}</option>
                <option value="any">{t('push.joinAny')}</option>
              </Select>
            </div>
            {rows.map((r, i) => (
              // A grid rather than a wrapping flex row: `Input` sets
              // `w-full` itself, so a width class on it is a coin toss
              // over which utility Tailwind emits last — and the loser
              // was the layout, which put every field on its own line.
              // The track sizes belong to the row, not to the fields.
              <div
                key={i}
                className="grid grid-cols-[auto_1fr_auto] items-center gap-2 sm:grid-cols-[auto_10rem_auto_minmax(0,1fr)_auto]"
              >
                <Select
                  value={r.source}
                  onChange={(e) =>
                    edit(() =>
                      setRows(
                        rows.map((x, j) =>
                          j === i ? { ...x, source: e.target.value as Source } : x,
                        ),
                      ),
                    )
                  }
                >
                  <option value="trait">{t('push.sourceTrait')}</option>
                  <option value="device">{t('push.sourceDevice')}</option>
                  <option value="user">{t('push.sourceUser')}</option>
                  <option value="issue">{t('push.sourceIssue')}</option>
                </Select>
                {valueOnly(r) ? (
                  <div className="sm:col-span-3">
                    <Input
                      value={r.value}
                      placeholder={
                        r.source === 'issue'
                          ? t('push.issueIdPlaceholder')
                          : t('push.appUserIdPlaceholder')
                      }
                      onChange={(e) =>
                        edit(() =>
                          setRows(
                            rows.map((x, j) => (j === i ? { ...x, value: e.target.value } : x)),
                          ),
                        )
                      }
                    />
                  </div>
                ) : (
                  <>
                    <Input
                      value={r.key}
                      placeholder={t('push.attributePlaceholder')}
                      onChange={(e) =>
                        edit(() =>
                          setRows(rows.map((x, j) => (j === i ? { ...x, key: e.target.value } : x))),
                        )
                      }
                    />
                    <Select
                      value={r.op}
                      onChange={(e) =>
                        edit(() =>
                          setRows(
                            rows.map((x, j) => (j === i ? { ...x, op: e.target.value as Op } : x)),
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
                    {r.op === 'exists' ? (
                      // The cell still has to exist, or the remove
                      // button slides under the operator.
                      <span />
                    ) : (
                      <Input
                        value={r.value}
                        placeholder={
                          r.op === 'in' ? t('push.valueListPlaceholder') : t('push.valuePlaceholder')
                        }
                        onChange={(e) =>
                          edit(() =>
                            setRows(
                              rows.map((x, j) => (j === i ? { ...x, value: e.target.value } : x)),
                            ),
                          )
                        }
                      />
                    )}
                  </>
                )}
                <button
                  type="button"
                  aria-label={t('push.removeCondition')}
                  className="px-1 text-fg-subtle hover:text-kind-error"
                  onClick={() => edit(() => setRows(rows.filter((_, j) => j !== i)))}
                >
                  ×
                </button>
              </div>
            ))}
            <div>
              <Button size="sm" onClick={() => edit(() => setRows([...rows, blank()]))}>
                {t('push.addCondition')}
              </Button>
            </div>
          </div>
        )}

        <div
          data-audience-panel
          className="flex flex-wrap items-center gap-3 border-t border-border/60 px-3.5 py-2.5"
        >
          <Button size="sm" disabled={busy || !request} onClick={runPreview}>
            {t('push.previewAudience')}
          </Button>
          {rawError && <span className="text-xs text-kind-error">{rawError}</span>}
          {preview && <Stat value={preview.matched} label={t('push.audienceMatched')} />}
          {preview === null && !rawError && (
            <span className="text-xs text-fg-subtle">{t('push.audienceNotCounted')}</span>
          )}
        </div>
      </Panel>

      {preview && preview.sample.length > 0 && (
        <Panel title={t('push.audienceSample')}>
          <DataTable
            rowKey={(d) => d.id}
            columns={[
              {
                key: 'provider',
                label: t('push.provider'),
                width: '110px',
                render: (d) => <span className="font-mono text-xs">{d.provider}</span>,
              },
              {
                key: 'traits',
                label: t('push.traits'),
                render: (d) => <span className="font-mono text-xs">{summarise(d.traits)}</span>,
              },
              {
                key: 'metadata',
                label: t('push.deviceFacts'),
                render: (d) => <span className="font-mono text-xs">{summarise(d.metadata)}</span>,
              },
              {
                key: 'user',
                label: t('push.identity'),
                width: '120px',
                render: (d) => (
                  <span className="font-mono text-xs text-fg-subtle">
                    {d.userKeyTail ? `···${d.userKeyTail}` : '—'}
                  </span>
                ),
              },
            ]}
            rows={preview.sample}
          />
        </Panel>
      )}

      <Panel title={t('push.audienceMessage')}>
        <div className="flex flex-wrap items-end gap-3 p-3.5">
          <Field label={t('push.testSubject')} className="min-w-0 flex-1">
            <Input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder={t('push.testSubjectPlaceholder')}
            />
          </Field>
          <Field label={t('push.testBody')} className="min-w-0 flex-1">
            <Input
              value={body}
              onChange={(e) => setBody(e.target.value)}
              placeholder={t('push.testBodyPlaceholder')}
            />
          </Field>
          <Button
            variant="primary"
            size="sm"
            disabled={busy || !request || preview === null || preview.matched === 0 || title.trim() === ''}
            onClick={() => {
              if (!request || preview === null) return;
              // The number in the confirmation is the number the send
              // carries. If the audience has grown since, the server
              // refuses rather than reaching more people than this
              // sentence promised.
              const ok = window.confirm(
                t('push.confirmSend').replace('{n}', String(preview.matched)),
              );
              if (!ok) return;
              setBusy(true);
              setError(null);
              void api
                .sendToAudience(projectId, request, title, body, preview.matched, sendKey)
                .then((r) => setSent(r.alreadySent ? -1 : r.queued))
                .catch((e: Error) => {
                  setError(e.message);
                  setPreview(null);
                })
                .finally(() => setBusy(false));
            }}
          >
            {preview === null
              ? t('push.previewFirst')
              : t('push.sendToAudience').replace('{n}', String(preview.matched))}
          </Button>
        </div>
        {error && <ErrorBanner>{error}</ErrorBanner>}
        {sent !== null && (
          <div className="border-t border-border/60 px-3.5 py-2 text-xs text-fg-muted">
            {sent < 0
              ? t('push.audienceAlreadySent')
              : t('push.audienceQueued').replace('{n}', String(sent))}
          </div>
        )}
      </Panel>
    </div>
  );
}


// ── integrate ───────────────────────────────────────────────────────

/// The exact call, with this deployment's URL already in it.
///
/// The whole API is one POST with three fields, which is why there is
/// no server SDK to install. What an integrator needs is not a
/// dependency — it is the call, and to know which of the two token
/// scopes it takes, because the one their app already ships with is
/// the wrong one and the failure is a 403 they read as an outage.

function IntegrateSection() {
  const t = useT();
  const [lang, setLang] = useState<SnippetLang>('go');
  const base = window.location.origin;

  return (
    <div className="flex flex-col gap-3">
      <Panel title={t('push.integrateEndpoint')}>
        {/* Two rows of one spec, not a paragraph about a spec. The
            method and the scope are both constraints on the same call,
            so they read as the same shape: a mono label in the gutter,
            the value beside it. */}
        <div className="grid grid-cols-[auto_1fr] items-baseline gap-x-3 gap-y-2 p-3.5">
          <span className="font-mono text-[11px] uppercase tracking-wide text-fg-subtle">
            POST
          </span>
          <div className="min-w-0">
            <Copyable text={`${base}${SEND_PATH}`} />
          </div>

          <span className="font-mono text-[11px] uppercase tracking-wide text-fg-subtle">
            SCOPE
          </span>
          <div className="flex min-w-0 flex-wrap items-baseline gap-x-2.5 gap-y-1 text-xs">
            <code className="font-mono text-fg">api</code>
            {/* The likely mistake is reaching for the token the app
                already ships with. It authenticates and then 403s on
                this route, which reads as an outage. */}
            <span className="text-fg-subtle">{t('push.integrateScopeNote')}</span>
            {/* Settings ▸ Tokens, not Push ▸ Credentials. Those are
                the vendor's keys; this is the token that authorises
                the call, and sending someone to the wrong one is
                worse than sending them nowhere. */}
            <Link to="/settings?tab=tokens" className="text-accent hover:underline">
              {t('push.integrateMint')} →
            </Link>
          </div>
        </div>
      </Panel>

      <Panel
        title={t('push.integrateSend')}
        action={
          <div className="flex flex-wrap gap-1">
            {SNIPPET_LANGS.map((l) => (
              <button
                key={l.id}
                type="button"
                onClick={() => setLang(l.id)}
                className={clsx(
                  'rounded px-1.5 py-0.5 font-mono text-[11px] transition-colors',
                  lang === l.id
                    ? 'bg-raised text-fg'
                    : 'text-fg-subtle hover:text-fg-muted',
                )}
              >
                {l.label}
              </button>
            ))}
          </div>
        }
      >
        <CodeBlock text={snippet(lang, base)} language={HLJS_LANG[lang]} />
      </Panel>

      <Panel title={t('push.integratePoll')}>
        <p className="px-3.5 pt-3 text-xs text-fg-muted">{t('push.integratePollWhy')}</p>
        <CodeBlock text={pollSnippet(base)} language="bash" />
      </Panel>

      <Panel title={t('push.integrateCount')}>
        <p className="px-3.5 pt-3 text-xs text-fg-muted">{t('push.integrateCountWhy')}</p>
        <CodeBlock text={countSnippet(base)} language="bash" />
      </Panel>
    </div>
  );
}

/// A block of code, lit by its own grammar, and a way to take it.
function CodeBlock({ text, language }: { language: string; text: string }) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  return (
    <div className="relative">
      {/* The grammar's own escaped output, over text this module
          holds as a constant. `devtools/check-highlight.mjs` strips
          the tags back off and compares. */}
      <pre
        className="overflow-x-auto px-3.5 py-3 font-mono text-xs leading-relaxed text-fg"
        dangerouslySetInnerHTML={{ __html: highlightBlock(text, language) }}
      />
      <button
        type="button"
        title={copied ? t('identity.copied') : t('identity.copyHint')}
        className="absolute right-2 top-2 rounded border border-border bg-surface p-1 text-fg-subtle hover:text-fg"
        onClick={() => {
          void navigator.clipboard.writeText(text);
          setCopied(true);
          setTimeout(() => setCopied(false), 1200);
        }}
      >
        {copied ? <Check className="size-3.5 text-ok" /> : <Copy className="size-3.5" />}
      </button>
    </div>
  );
}

/// One value, and a way to take it.
function Copyable({ text }: { text: string }) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  return (
    <button
      type="button"
      title={copied ? t('identity.copied') : t('identity.copyHint')}
      className="group inline-flex items-center gap-1.5 font-mono text-sm text-fg"
      onClick={() => {
        void navigator.clipboard.writeText(text);
        setCopied(true);
        setTimeout(() => setCopied(false), 1200);
      }}
    >
      {text}
      {copied ? (
        <Check className="size-3 text-ok" />
      ) : (
        <Copy className="size-3 text-fg-subtle opacity-0 transition-opacity group-hover:opacity-100" />
      )}
    </button>
  );
}

// ── devices ─────────────────────────────────────────────────────────

function DevicesSection({ projectId }: { projectId: string }) {
  const t = useT();
  // Revoked rows are hidden by default and the toggle is here rather
  // than nowhere: a device that stopped receiving is exactly what
  // someone comes to this page to find, and `live` alone cannot show
  // it.
  const [scope, setScope] = useState<'all' | 'live'>('live');
  const [offset, setOffset] = useState(0);
  const devices = useAsyncData(
    () => api.pushDevices(projectId, PAGE, scope, offset),
    [projectId, scope, offset],
  );
  const rows = devices.data?.devices ?? [];
  const total = devices.data?.total ?? rows.length;

  return (
    <Panel
      title={`${t('push.devicesTitle')} (${total})`}
      action={
        <Select
          value={scope}
          onChange={(e) => {
            setScope(e.target.value as 'all' | 'live');
            setOffset(0);
          }}
          aria-label={t('push.filterScope')}
        >
          <option value="live">{t('push.scopeLive')}</option>
          <option value="all">{t('push.scopeAll')}</option>
        </Select>
      }
    >
      <DataTable
        rows={rows}
        rowKey={(d) => d.id}
        empty={t('push.devicesEmpty')}
        columns={[
          {
            key: 'device',
            label: t('push.device'),
            render: (d) => (
              <span className="font-mono text-xs">
                {d.provider}
                {d.env ? `/${d.env}` : ''}
                <span className="text-fg-subtle"> ···{d.tokenTail ?? ''}</span>
                {d.revokedAt && <span className="text-fg-subtle"> {t('push.revoked')}</span>}
                {/* The counter existed, was written on every transient
                    failure, and nothing anywhere read it — the module
                    that keeps it claimed a quarantine rule that did not
                    exist. It is a fact about one device, so it belongs
                    next to that device. */}
                {!d.revokedAt && d.badStreak > 0 && (
                  <span
                    title={t('push.badStreakTitle').replace('{n}', String(d.badStreak))}
                    className="ml-1.5 rounded-sm bg-kind-warn/15 px-1 text-kind-warn tabular-nums"
                  >
                    {t('push.badStreak').replace('{n}', String(d.badStreak))}
                  </span>
                )}
              </span>
            ),
          },
          {
            // The identity key, not a word for whether there is one.
            // This column said "addressable / not", which asks
            // whether `sentori.user()` ran before `register()` — and
            // two people read the source to find that out. A key or a
            // dash says it without a word.
            key: 'user',
            label: t('push.user'),
            width: '150px',
            render: (d) =>
              d.addressable ? (
                <span className="font-mono text-xs text-fg-muted">···{d.userKeyTail ?? ''}</span>
              ) : (
                <span className="text-xs text-fg-subtle" title={t('push.noUserHint')}>
                  {t('push.noUser')}
                </span>
              ),
          },
          {
            key: 'metadata',
            label: t('push.metadata'),
            render: (d) =>
              Object.keys(d.metadata ?? {}).length === 0 ? (
                <span className="text-xs text-fg-subtle">{t('push.metadataNone')}</span>
              ) : (
                // `k=v  k=v`, the same as the column beside it. Raw
                // JSON next to a summarised column reads as two kinds
                // of data rather than two sources of the same kind.
                <span className="font-mono text-xs text-fg-muted">{summarise(d.metadata)}</span>
              ),
          },
          {
            // What a campaign's conditions are matched against. The
            // server started returning it and nothing showed it, which
            // is the same defect as `metadata` before 2.22.0: an
            // integrator passing traits had no way to see them arrive,
            // and no way to tell a condition that matches nothing from
            // one whose data never came.
            key: 'traits',
            label: t('push.traits'),
            render: (d) =>
              Object.keys(d.traits ?? {}).length === 0 ? (
                <span className="text-xs text-fg-subtle">{t('push.traitsNone')}</span>
              ) : (
                <span className="font-mono text-xs text-fg-muted">{summarise(d.traits)}</span>
              ),
          },
          {
            key: 'lastSeenAt',
            label: t('settings.colWhen'),
            width: '110px',
            align: 'right',
            render: (d) => (
              <span className="text-xs tabular-nums text-fg-subtle">
                {formatRelative(d.lastSeenAt)}
              </span>
            ),
          },
          {
            // Retiring a device needed either the app or a failed
            // delivery. Someone looking at one that should stop
            // receiving had nowhere to click.
            key: 'revoke',
            label: '',
            width: '70px',
            align: 'right',
            render: (d) =>
              d.revokedAt ? null : (
                <button
                  type="button"
                  className="text-xs text-fg-subtle hover:text-kind-error"
                  onClick={() => {
                    if (window.confirm(t('push.revokeConfirm'))) {
                      void api.revokePushDevice(projectId, d.id).then(devices.reload);
                    }
                  }}
                >
                  {t('push.revokeAction')}
                </button>
              ),
          },
        ]}
      />
      <Pager offset={offset} onOffset={setOffset} page={PAGE} shown={rows.length} total={total} />
    </Panel>
  );
}

// ── credentials ─────────────────────────────────────────────────────

/// What each provider actually needs, as fields rather than as JSON
/// somebody types by hand.
///
/// The form used to be one text box labelled "Config (JSON)" and one
/// labelled "Secret", and it was wrong in three ways at once. The
/// secret was an `<input>`, which strips line breaks — so a pasted
/// `.p8` arrived as one long line and stopped being a PEM. The JSON
/// example named `bundleId`, and the worker reads `topic`, so
/// following the example exactly produced `topic missing` on the
/// first send. And nothing was checked at save, so both surfaced
/// hours later as a notification that did not arrive.
///
/// `secretKey` is the field whose value goes to `secret_blob`;
/// everything else is merged into the non-secret `config` the worker
/// reads by these exact names.
// Every string is a literal, not a template. `t` takes a union of
// the keys that exist, so a key assembled at runtime does not
// typecheck — which is the compiler catching a label that would have
// rendered as its own key on a screen nobody had opened yet.
type FieldSpec = {
  hint: MessageKey;
  key: string;
  label: MessageKey;
  multiline?: boolean;
  options?: { label: MessageKey; value: string }[];
  placeholder?: MessageKey;
  required?: boolean;
};

const PROVIDER_FIELDS: Record<string, FieldSpec[]> = {
  apns: [
    {
      key: 'keyId',
      label: 'push.field.keyId',
      hint: 'push.hint.apns.keyId',
      placeholder: 'push.placeholder.apns.keyId',
      required: true,
    },
    {
      key: 'teamId',
      label: 'push.field.teamId',
      hint: 'push.hint.apns.teamId',
      placeholder: 'push.placeholder.apns.teamId',
      required: true,
    },
    {
      // Sent as `topic`, because that is the name the worker reads.
      // The label says bundle id, which is what Apple calls it — and
      // the old placeholder said `bundleId`, which is what nothing
      // reads.
      key: 'topic',
      label: 'push.field.topic',
      hint: 'push.hint.apns.topic',
      placeholder: 'push.placeholder.apns.topic',
      required: true,
    },
    {
      key: 'production',
      label: 'push.field.production',
      hint: 'push.hint.apns.production',
      options: [
        { label: 'push.option.production', value: 'production' },
        { label: 'push.option.sandbox', value: 'sandbox' },
      ],
    },
    {
      key: 'secretKey',
      label: 'push.field.secretKey',
      hint: 'push.hint.apns.secretKey',
      placeholder: 'push.placeholder.apns.secretKey',
      multiline: true,
      required: true,
    },
  ],
  fcm: [
    {
      // Everything else is read out of the file itself.
      key: 'secretKey',
      label: 'push.field.secretKey',
      hint: 'push.hint.fcm.secretKey',
      placeholder: 'push.placeholder.fcm.secretKey',
      multiline: true,
      required: true,
    },
  ],
  webpush: [
    {
      key: 'subject',
      label: 'push.field.subject',
      hint: 'push.hint.webpush.subject',
      placeholder: 'push.placeholder.webpush.subject',
    },
    {
      key: 'vapidPublicKey',
      label: 'push.field.vapidPublicKey',
      hint: 'push.hint.webpush.vapidPublicKey',
      placeholder: 'push.placeholder.webpush.vapidPublicKey',
      required: true,
    },
    {
      key: 'secretKey',
      label: 'push.field.secretKey',
      hint: 'push.hint.webpush.secretKey',
      placeholder: 'push.placeholder.webpush.secretKey',
      multiline: true,
      required: true,
    },
  ],
};

/// Getting a credential in, for someone who has never done it.
///
/// The old form assumed you arrived holding the right file and
/// knowing what a Team ID was. Both assumptions fail in the same
/// direction: you paste something plausible, it saves, and you learn
/// that night. So this section is four things in order — where to get
/// it, how to recognise it, what we made of what you pasted, and what
/// the vendor said about it.
function CredentialsSection({ projectId }: { projectId: string }) {
  const t = useT();
  const creds = useAsyncData(() => api.pushCredentials(projectId), [projectId]);
  const [provider, setProvider] = useState<Provider>('apns');
  const configured = creds.data?.credentials ?? [];

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap gap-1.5">
        {PROVIDER_SPECS.map((s) => (
          <button
            key={s.provider}
            type="button"
            onClick={() => setProvider(s.provider)}
            className={`h-7 rounded border px-2.5 font-mono text-xs transition-colors ${
              provider === s.provider
                ? 'border-accent/60 bg-accent/10 text-accent'
                : 'border-border/60 text-fg-muted hover:text-fg'
            }`}
          >
            {t(s.title as MessageKey)}
          </button>
        ))}
      </div>

      <ProviderGuide provider={provider} />
      <AddCredential projectId={projectId} provider={provider} onSaved={creds.reload} />

      <Panel title={`${t('push.credentialsTitle')} (${configured.length})`}>
        {configured.length === 0 ? (
          <PanelEmpty>{t('push.credentialsEmpty')}</PanelEmpty>
        ) : (
          <div className="divide-y divide-border/60">
            {configured.map((c) => (
              <CredentialRow key={c.id} cred={c} projectId={projectId} onChange={creds.reload} />
            ))}
          </div>
        )}
      </Panel>
    </div>
  );
}

/// Where the file comes from and how to tell it from its twin.
///
/// The twin is the point. Apple's two `.p8` files are the same shape
/// and the same name; Firebase's two JSON downloads sit one screen
/// apart. Naming the wrong file is most of what this panel is for —
/// "invalid credential" sends someone back to the button they already
/// pressed.
function ProviderGuide({ provider }: { provider: Provider }) {
  const t = useT();
  const spec = PROVIDER_SPECS.find((s) => s.provider === provider);
  if (!spec) return null;

  return (
    <Panel title={t('push.specHowTo')}>
      <div className="flex flex-col gap-3 p-3.5">
        <ol className="flex flex-col gap-1.5">
          {spec.steps.map((step, i) => (
            <li key={step} className="flex gap-2.5 text-xs leading-relaxed">
              <span className="w-3 shrink-0 pt-px text-right font-mono tabular-nums text-fg-subtle">
                {i + 1}
              </span>
              <span className="text-fg-muted">{t(step as MessageKey)}</span>
            </li>
          ))}
        </ol>

        {spec.href !== undefined && (
          <a
            href={spec.href}
            target="_blank"
            rel="noreferrer noopener"
            className="flex items-center gap-1.5 self-start font-mono text-xs text-accent hover:underline"
          >
            <ExternalLink aria-hidden className="size-3" />
            {spec.href}
          </a>
        )}

        {/* What the download looks like, so it can be recognised
            before it is pasted. Not translated: a localised file name
            is a file name nobody finds. */}
        <div className="grid grid-cols-[auto_1fr] items-baseline gap-x-3 gap-y-1 border-t border-border/60 pt-3">
          {spec.spec.map((row) => (
            <Fragment key={row.label + row.value}>
              <span className="font-mono text-[11px] uppercase tracking-wide text-fg-subtle">
                {t(row.label as MessageKey)}
              </span>
              <code className="min-w-0 break-all font-mono text-xs text-fg">{row.value}</code>
            </Fragment>
          ))}
        </div>
      </div>
    </Panel>
  );
}

/// The form, with the file read for you.
///
/// Two of the four APNs fields are typed by hand off a web page,
/// which is where transposed characters come from. The Key ID is in
/// the file name; Firebase's project is in the file. Both are lifted
/// rather than asked for.
function AddCredential({
  onSaved,
  projectId,
  provider,
}: {
  onSaved: () => void;
  projectId: string;
  provider: Provider;
}) {
  const t = useT();
  const [values, setValues] = useState<Record<string, string>>({});
  const [filename, setFilename] = useState('');
  const [busy, setBusy] = useState(false);
  const [saveError, setSaveError] = useState<null | string>(null);
  const [verdict, setVerdict] = useState<null | ProbeVerdict>(null);

  const fields = PROVIDER_FIELDS[provider] ?? [];
  const val = (k: string) => values[k] ?? '';
  const missing = fields.filter((f) => f.required && val(f.key).trim().length === 0);
  const set = (k: string, v: string) => setValues((prev) => ({ ...prev, [k]: v }));

  // Runs on every change of the secret. Pure, so it costs nothing and
  // cannot be out of date with what is in the box.
  const seen = recognise(val('secretKey'), filename);

  const reset = () => {
    setValues({});
    setFilename('');
    setSaveError(null);
    setVerdict(null);
  };

  const take = (name: string, text: string) => {
    const r = recognise(text, name);
    setFilename(name);
    setVerdict(null);
    setSaveError(null);
    setValues((prev) => ({
      ...prev,
      secretKey: text,
      // Only ever fills a blank. Overwriting something typed would be
      // the console arguing with the operator.
      ...(r.keyId !== undefined && (prev.keyId ?? '') === '' ? { keyId: r.keyId } : {}),
    }));
  };

  return (
    <Panel title={t('push.credentialsAdd')}>
      <div className="flex flex-col gap-3 p-3.5">
        <div className="flex flex-wrap items-center gap-2.5">
          <label className={`${buttonClass('secondary', 'sm')} cursor-pointer`}>
            <Upload aria-hidden className="size-3.5" />
            {t('push.chooseFile')}
            <input
              type="file"
              className="hidden"
              onChange={(e) => {
                const file = e.target.files?.[0];
                if (!file) return;
                void file.text().then((text) => take(file.name, text));
                // So choosing the same file twice still fires.
                e.target.value = '';
              }}
            />
          </label>
          {filename !== '' && (
            <code className="min-w-0 break-all font-mono text-xs text-fg-muted">{filename}</code>
          )}
        </div>

        {/* What we made of it, before anything is saved. The wrong
            file is named here rather than becoming a validation error
            that points at the right field for the wrong reason. */}
        {seen.issue !== undefined && (
          <div className="flex gap-2 text-xs leading-relaxed text-kind-error">
            <CircleX aria-hidden className="size-3.5 shrink-0 translate-y-0.5" />
            <span>{t(`push.secretIssue.${seen.issue}` as MessageKey)}</span>
          </div>
        )}
        {seen.issue === undefined && seen.provider !== undefined && (
          <div className="flex flex-wrap items-baseline gap-x-2.5 gap-y-1 text-xs">
            <span className="flex items-baseline gap-1.5 text-kind-probe">
              <CircleCheck aria-hidden className="size-3 shrink-0 translate-y-0.5" />
              {t(`push.secretSeen.${seen.provider}` as MessageKey)}
            </span>
            {seen.provider !== provider && (
              <span className="text-kind-warn">{t('push.secretWrongTab')}</span>
            )}
            {seen.keyId !== undefined && (
              <span className="font-mono text-fg-subtle">{t('push.keyIdFromFilename')}</span>
            )}
            {seen.projectId !== undefined && (
              <code className="font-mono text-fg-subtle">{seen.projectId}</code>
            )}
          </div>
        )}

        <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-4">
          {fields
            .filter((f) => !f.multiline)
            .map((f) => (
              <Field
                key={f.key}
                label={
                  f.required ? (
                    <>
                      {t(f.label)}
                      <span
                        aria-label={t('push.required')}
                        title={t('push.required')}
                        className="ml-1 text-kind-warn"
                      >
                        •
                      </span>
                    </>
                  ) : (
                    t(f.label)
                  )
                }
              >
                {f.options ? (
                  <Select
                    value={val(f.key) || f.options[0]?.value}
                    onChange={(e) => set(f.key, e.target.value)}
                  >
                    {f.options.map((o) => (
                      <option key={o.value} value={o.value}>
                        {t(o.label)}
                      </option>
                    ))}
                  </Select>
                ) : (
                  <Input
                    value={val(f.key)}
                    onChange={(e) => set(f.key, e.target.value)}
                    placeholder={f.placeholder ? t(f.placeholder) : undefined}
                  />
                )}
              </Field>
            ))}
          <Field label={t('push.credentialLabel')}>
            <Input
              value={val('label')}
              onChange={(e) => set('label', e.target.value)}
              placeholder={t('push.credentialLabelPlaceholder')}
            />
          </Field>
        </div>

        {fields
          .filter((f) => f.multiline)
          .map((f) => (
            <Field key={f.key} label={t(f.label)}>
              {/* A textarea, because a PEM and a service-account file
                  both have line breaks and an `<input>` deletes them. */}
              <Textarea
                value={val(f.key)}
                onChange={(e) => {
                  set(f.key, e.target.value);
                  setVerdict(null);
                }}
                rows={provider === 'fcm' ? 8 : 6}
                placeholder={f.placeholder ? t(f.placeholder) : undefined}
              />
              <p className="mt-1 text-xs leading-snug text-fg-subtle">{t(f.hint)}</p>
            </Field>
          ))}

        <div className="flex flex-wrap items-center gap-3">
          <Button
            variant="primary"
            size="sm"
            disabled={busy || missing.length > 0}
            onClick={() => {
              setBusy(true);
              setSaveError(null);
              setVerdict(null);
              const config: Record<string, unknown> = {};
              for (const f of fields) {
                if (f.key === 'secretKey') continue;
                const v = val(f.key).trim();
                if (v.length === 0) continue;
                // The one field that is not a string: the worker
                // reads `production` as a boolean.
                config[f.key] = f.key === 'production' ? v === 'production' : v;
              }
              void api
                .addPushCredential(
                  projectId,
                  provider,
                  config,
                  val('secretKey'),
                  val('label').trim() || undefined,
                )
                // Saving and asking are one action. A credential that
                // saved and was never probed is exactly the state
                // this section exists to abolish.
                .then((r) => api.probePushCredential(projectId, r.id))
                .then((v) => {
                  setVerdict(v);
                  if (v.status === 'ok') reset();
                  onSaved();
                })
                .catch((e: Error) =>
                  setSaveError(
                    e instanceof ApiError && e.code
                      ? credentialProblem(t, { code: e.code, field: e.field ?? null })
                      : e.message,
                  ),
                )
                .finally(() => setBusy(false));
            }}
          >
            {t('push.saveAndProbe')}
          </Button>
          {missing.length > 0 && (
            <span className="font-mono text-xs tabular-nums text-fg-subtle">
              {t('push.missingCount').replace('{n}', String(missing.length))}
            </span>
          )}
        </div>
      </div>
      {saveError !== null && <ErrorBanner>{saveError}</ErrorBanner>}
      {verdict !== null && (
        <div className="border-t border-border/60 px-3.5 py-2.5">
          <VerdictLine verdict={verdict} />
        </div>
      )}
    </Panel>
  );
}

/// What the vendor said, in one line plus its own words.
function VerdictLine({ verdict }: { verdict: ProbeVerdict }) {
  const t = useT();
  const Icon =
    verdict.status === 'ok' ? CircleCheck : verdict.status === 'rejected' ? CircleX : Info;
  return (
    <div className={`flex gap-2 text-xs leading-relaxed ${verdictTone(verdict.status)}`}>
      <Icon aria-hidden className="size-3.5 shrink-0 translate-y-0.5" />
      <div className="min-w-0 space-y-0.5">
        <div>{t(`push.verdict.${verdict.status}` as MessageKey)}</div>
        {/* The vendor's own words. A verdict of "limited" with no
            reason is a worse answer than no verdict. */}
        {verdict.detail !== null && verdict.detail !== '' && (
          <div className="break-words text-fg-subtle">{verdict.detail}</div>
        )}
      </div>
    </div>
  );
}

/// One stored credential: whether it sends, what we last heard, and
/// the two things that can be done to it.
function CredentialRow({
  cred,
  onChange,
  projectId,
}: {
  cred: PushCredential;
  onChange: () => void;
  projectId: string;
}) {
  const t = useT();
  const [busy, setBusy] = useState(false);
  const [verdict, setVerdict] = useState<null | ProbeVerdict>(null);
  const status = verdict?.status ?? cred.last_validate_status ?? null;
  const detail = verdict?.detail ?? cred.last_validate_detail ?? null;
  const active = cred.active !== false;

  const run = (p: Promise<unknown>) => {
    setBusy(true);
    void p.then(onChange).finally(() => setBusy(false));
  };

  return (
    <div className="flex flex-wrap items-start gap-x-3 gap-y-2 px-3.5 py-2.5 text-sm">
      {/* Sending or staged, as a state rather than a sentence. */}
      <span
        className={`w-16 shrink-0 font-mono text-[11px] uppercase tracking-wide ${
          active ? 'text-kind-probe' : 'text-fg-subtle'
        }`}
      >
        {t(active ? 'push.credActive' : 'push.credStaged')}
      </span>
      <div className="min-w-0 flex-1 space-y-1">
        <div className="flex flex-wrap items-baseline gap-x-2.5 gap-y-1">
          <span className="font-mono text-xs text-fg">{cred.kind}</span>
          {cred.label !== null && cred.label !== undefined && cred.label !== '' && (
            <span className="text-xs text-fg-muted">{cred.label}</span>
          )}
        </div>
        {/* What the server understood, not what was typed. An
            operator with two Firebase projects or two Apple teams has
            no other way to see which one they pasted. */}
        <div className="break-all font-mono text-xs text-fg-muted">
          {summarise(cred.config) || t('push.metadataNone')}
        </div>
        {cred.problem ? (
          <div className="flex items-baseline gap-1.5 text-xs text-kind-error">
            <CircleX
              aria-label={t('push.unusable')}
              role="img"
              className="size-3 shrink-0 translate-y-0.5"
            />
            <span>{credentialProblem(t, cred.problem)}</span>
          </div>
        ) : status !== null ? (
          <VerdictLine
            verdict={{
              status: status as ProbeVerdict['status'],
              code: null,
              field: null,
              detail,
              safeToActivate: status === 'ok',
            }}
          />
        ) : (
          <div className="text-xs text-fg-subtle">{t('push.neverProbed')}</div>
        )}
      </div>
      <div className="flex shrink-0 items-center gap-2.5 text-xs">
        <button
          type="button"
          disabled={busy}
          className="text-fg-subtle hover:text-fg disabled:opacity-50"
          onClick={() =>
            run(api.probePushCredential(projectId, cred.id).then((v) => setVerdict(v)))
          }
        >
          {t('push.reprobe')}
        </button>
        {!active && (
          <button
            type="button"
            disabled={busy}
            className="text-accent hover:underline disabled:opacity-50"
            onClick={() => {
              // The server refuses a known-bad verdict without
              // `force`; the confirm is where the operator sees which
              // verdict they are overriding, rather than a checkbox
              // that gets ticked out of habit.
              const bad = status === 'rejected' || status === 'limited';
              if (bad && !window.confirm(t('push.activateAnyway', { status: status ?? '' }))) {
                return;
              }
              run(api.activatePushCredential(projectId, cred.id, bad));
            }}
          >
            {t('push.activate')}
          </button>
        )}
        <button
          type="button"
          disabled={busy}
          className="text-fg-subtle hover:text-kind-error disabled:opacity-50"
          onClick={() => {
            // Deleting the one that sends stops push. Say so, rather
            // than asking the generic question.
            const key = active ? 'push.deleteActiveConfirm' : 'push.deleteConfirm';
            if (window.confirm(t(key, { kind: cred.kind }))) {
              run(api.deletePushCredential(projectId, cred.id));
            }
          }}
        >
          {t('common.delete')}
        </button>
      </div>
    </div>
  );
}

/** The non-secret facts, as `key=value`, in the order they were
 *  stored. Raw JSON with braces reads as debug output; this is the
 *  same information a person can scan. */
function summarise(config: unknown): string {
  if (!config || typeof config !== 'object') return '';
  return Object.entries(config as Record<string, unknown>)
    .map(([k, v]) => `${k}=${String(v)}`)
    .join('  ');
}
