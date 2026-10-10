import { useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { Activity, Plus } from 'lucide-react';
import { Heading } from '../../shared/design/Surface';
import { Badge, Button, Field, Input } from '../../shared/design/controls';
import { Dialog } from '../../shared/design/Dialog';
import { useShell } from '../App';
import { ErrorBanner, TimeAgo } from '../components/ui';
import { useT } from '../i18n';
import { api, ApiError } from '../lib/api';
import { commandId } from '../lib/team-contract';
import { formatApiError } from '../lib/useAsyncData';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';
import type { Monitor, MonitorCommand, MonitorConfiguration } from '../lib/monitor-contract';

function useCommand(project: string, saved: () => void) {
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null), [uncertain, setUncertain] = useState(false);
  const lock = useRef(false), pending = useRef<MonitorCommand | null>(null);
  async function run(command: MonitorCommand) {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError(null); pending.current ??= command;
    try { await api.monitorCommand(project, pending.current); pending.current = null; setUncertain(false); saved(); }
    catch (failure) { if (failure instanceof ApiError && failure.status >= 400 && failure.status < 500) { pending.current = null; setUncertain(false); } else setUncertain(true); setError(formatApiError(failure)); }
    finally { lock.current = false; setBusy(false); }
  }
  return { busy, error, uncertain, run, retry: () => { if (pending.current) void run(pending.current); } };
}
export default function MonitorsPage() {
  const { activeProject, me } = useShell(), t = useT();
  return activeProject ? <Monitors key={activeProject.id} project={activeProject.id} owner={me.role === 'superadmin'}/> : <p className="p-6">{t('team.chooseProject')}</p>;
}
function Monitors({ project, owner }: { project: string; owner: boolean }) {
  const t = useT(), [params] = useSearchParams(), list = useScopedAsyncData(project, () => api.listMonitors(project));
  const [editor, setEditor] = useState<Monitor | 'new' | null>(null);
  return <div className="team-page p-4 md:p-6"><Heading title={t('monitor.title')} description={t('monitor.description')} action={owner && <Button variant="primary" onClick={() => setEditor('new')}><Plus size={15}/>{t('monitor.create')}</Button>}/>
    <div className="team-toolbar"><p className="muted text-sm">{t('monitor.publicOnly')}</p><Button onClick={list.reload}>{t('team.refresh')}</Button></div>
    {list.error && <ErrorBanner reason={list.error}>{t('team.failed')}</ErrorBanner>}{list.loading && <p role="status">{t('shell.loading')}</p>}
    {list.data && (list.data.monitors.length ? <div className="team-monitor-list">{list.data.monitors.map(m => <MonitorCard key={`${m.id}:${m.version}`} monitor={m} owner={owner} highlighted={params.get('id') === m.id} onEdit={() => setEditor(m)} reload={list.reload}/>)}</div> : <p>{t('monitor.empty')}</p>)}
    {editor && <Editor key={editor === 'new' ? 'new' : editor.id} project={project} monitor={editor === 'new' ? null : editor} onClose={() => setEditor(null)} saved={() => { setEditor(null); list.reload(); }}/>}
  </div>;
}
function MonitorCard({ monitor: m, owner, highlighted, onEdit, reload }: { monitor: Monitor; owner: boolean; highlighted: boolean; onEdit: () => void; reload: () => void }) {
  const t = useT(), last = m.samples.at(-1), command = useCommand(m.projectId, reload), [historyOpen, setHistoryOpen] = useState(false);
  const labels = { unknown: t('monitor.unknown'), operational: t('monitor.operational'), degraded: t('monitor.degraded'), down: t('monitor.down') };
  return <section className="team-monitor" data-monitor-id={m.id} data-highlighted={highlighted}><header><div className="team-monitor-identity"><span className="team-monitor-signal" aria-hidden><span className="team-health-dot" data-state={m.enabled ? m.status : 'unknown'}/></span><div><h2>{m.name}</h2><code>{m.url}</code></div></div><div className="team-monitor-controls"><Badge tone={!m.enabled ? 'neutral' : m.status === 'operational' ? 'success' : m.status === 'down' ? 'danger' : m.status === 'degraded' ? 'warning' : 'neutral'}>{m.enabled ? labels[m.status] : t('monitor.paused')}</Badge>{owner && <Button small onClick={onEdit}>{t('monitor.edit')}</Button>}</div></header>
    <div className="team-monitor-metrics"><div><small>{t('monitor.availability')}</small><strong>{m.availability === null ? '—' : `${m.availability.toFixed(1)}%`}</strong></div><div><small>{t('monitor.latency')}</small><strong>{last ? `${last.latency} ms` : '—'}</strong></div><div><small>{t('monitor.lastCheck')}</small>{last ? <TimeAgo iso={last.at}/> : <strong>—</strong>}</div></div>
    <div className="team-uptime-bars" role="img" aria-label={`${t('monitor.samples')}: ${m.samples.length}`}>{m.samples.map((s, i) => <span key={`${s.at}:${i}`} data-state={s.ok ? 'operational' : 'down'} title={`${s.at} · ${s.code ?? s.error} · ${s.latency} ms`}/>)}</div>
    <p className="muted text-xs">{t('monitor.window', { count: String(m.checks30d) })}</p>{m.incidentOpen && <p role="status" className="text-danger mt-3">{t('monitor.incident')}</p>}
    <footer><span>{m.intervalSeconds / 60} min · {t('monitor.failureThreshold')}: {m.failureThreshold}</span>{owner && <Button small busy={command.busy} disabled={!m.enabled} onClick={() => command.uncertain ? command.retry() : void command.run({ commandId: commandId(), id: m.id, expectedVersion: m.version, action: 'check', configuration: null })}><Activity size={14}/>{command.uncertain ? t('common.retry') : t('monitor.checkNow')}</Button>}<Link className="text-link" to={`/tasks?project=${m.projectId}&new=task`}>{t('team.create')}</Link></footer>
    {command.error && <ErrorBanner reason={command.error}>{t('team.failed')}</ErrorBanner>}
    <details className="team-monitor-history"><summary>{t('monitor.samples')}</summary>{m.samples.length ? <div className="team-check-log">{m.samples.slice(-12).reverse().map((s, i) => <div key={`${s.at}:${i}`}><TimeAgo iso={s.at}/><Badge tone={s.ok ? 'success' : 'danger'}>{s.ok ? t('monitor.operational') : t('monitor.down')}</Badge><code>{s.code === null ? s.error : `HTTP ${s.code}`}</code><span>{s.latency} ms</span></div>)}</div> : <p>{t('monitor.noHistory')}</p>}</details>
    <details className="team-monitor-history" onToggle={e => setHistoryOpen(e.currentTarget.open)}><summary>{t('monitor.incidents')}</summary>{historyOpen && <History project={m.projectId} id={m.id}/>}</details>
  </section>;
}
function History({ project, id }: { project: string; id: string }) {
  const t = useT(), [offset, setOffset] = useState(0), history = useScopedAsyncData(`${project}:${id}:${offset}`, () => api.monitorHistory(project, id, offset));
  return <>{history.error && <ErrorBanner reason={history.error}>{t('team.failed')}</ErrorBanner>}{history.data && <><div className="team-check-log">{history.data.events.map(event => <div key={event.id}><TimeAgo iso={event.at}/><Badge tone={event.kind === 'monitor.down' ? 'danger' : 'success'}>{event.kind === 'monitor.down' ? t('monitor.down') : t('monitor.recovered')}</Badge><code>{event.incidentId.slice(0, 8)}</code></div>)}</div>{!history.data.events.length && <p>{t('monitor.noHistory')}</p>}<div className="flex gap-2 mt-3"><Button small disabled={!offset} onClick={() => setOffset(Math.max(0, offset - 50))}>{t('queue.previous')}</Button><Button small disabled={history.data.nextOffset === null} onClick={() => setOffset(history.data!.nextOffset!)}>{t('queue.next')}</Button></div></>}</>;
}
function Editor({ project, monitor, onClose, saved }: { project: string; monitor: Monitor | null; onClose: () => void; saved: () => void }) {
  const t = useT(), [id] = useState(() => monitor?.id ?? commandId()), [deleting, setDeleting] = useState(false), command = useCommand(project, saved);
  const [draft, setDraft] = useState<MonitorConfiguration>(() => monitor ? { name: monitor.name, url: monitor.url, intervalSeconds: monitor.intervalSeconds, timeoutSeconds: monitor.timeoutSeconds, expectedStatus: monitor.expectedStatus, failureThreshold: monitor.failureThreshold, enabled: monitor.enabled } : { name: '', url: '', intervalSeconds: 300, timeoutSeconds: 10, expectedStatus: 200, failureThreshold: 2, enabled: true });
  const field = <K extends keyof MonitorConfiguration>(key: K, value: MonitorConfiguration[K]) => setDraft(old => ({ ...old, [key]: value }));
  const locked = command.busy || command.uncertain;
  const submit = (action: 'save' | 'delete') => command.run({ commandId: commandId(), id, expectedVersion: monitor?.version ?? 0, action, configuration: action === 'save' ? draft : null });
  return <Dialog open title={t(monitor ? 'monitor.edit' : 'monitor.create')} closeLabel={t('common.close')} onClose={() => { if (!locked) onClose(); }} footer={<>{monitor && <Button variant="danger" disabled={locked} onClick={() => setDeleting(true)}>{t('common.delete')}</Button>}<Button disabled={locked} onClick={onClose}>{t('team.cancel')}</Button><Button variant="primary" type="submit" form="monitor-editor" busy={command.busy}>{command.uncertain ? t('common.retry') : t('team.save')}</Button></>}>
    <form id="monitor-editor" className="form-stack" onSubmit={e => { e.preventDefault(); if (command.uncertain) command.retry(); else void submit('save'); }}><fieldset className="form-stack min-w-0" disabled={locked}><Field label={t('monitor.name')}><Input required maxLength={100} value={draft.name} onChange={e => field('name', e.target.value)}/></Field><Field label={t('monitor.url')} hint={t('monitor.publicOnly')}><Input required type="url" maxLength={2048} value={draft.url} onChange={e => field('url', e.target.value)}/></Field><div className="team-form-grid">{(['intervalSeconds', 'timeoutSeconds', 'expectedStatus', 'failureThreshold'] as const).map(key => <Field key={key} label={t(`monitor.${key}`)} hint={key === 'expectedStatus' ? t('monitor.finalStatusHint') : undefined}><Input required type="number" min={key === 'expectedStatus' ? 200 : 1} max={key === 'intervalSeconds' ? 1440 : key === 'timeoutSeconds' ? 30 : key === 'expectedStatus' ? 599 : 10} value={key === 'intervalSeconds' ? draft[key] / 60 : draft[key]} onChange={e => field(key, Number(e.target.value) * (key === 'intervalSeconds' ? 60 : 1))}/></Field>)}</div><label className="flex gap-3 items-center"><input type="checkbox" checked={draft.enabled} onChange={e => field('enabled', e.target.checked)}/>{t('team.enabled')}</label></fieldset>
      <p className="muted text-sm">{t('monitor.editHint')}</p>{command.error && <ErrorBanner reason={command.error}>{t('team.failed')}</ErrorBanner>}{command.uncertain && <p role="status">{t('team.uncertain')}</p>}{monitor && command.error && !command.uncertain && <p>{t('team.conflict')}</p>}
      {deleting && <div role="alert" className="form-stack"><p>{t('monitor.deleteHint')}</p><Button variant="danger" busy={command.busy} disabled={command.uncertain} onClick={() => void submit('delete')}>{t('monitor.confirmDelete')}</Button></div>}
    </form>
  </Dialog>;
}
