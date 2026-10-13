import { Checkbox, Radio } from "../../shared/design/Checkbox";
import { useEffect, useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { Download } from 'lucide-react';
import { Heading, Surface } from '../../shared/design/Surface';
import { Badge, Button, Field, Input, Textarea } from '../../shared/design/controls';
import { Dialog } from '../../shared/design/Dialog';
import { ErrorBanner, TimeAgo, formatAbsolute } from '../components/ui';
import { useShell } from '../App';
import { useT } from '../i18n';
import { api, ApiError } from '../lib/api';
import { canUse, type InstanceInfo } from '../lib/system-contract';
import { parseTransferDescriptor, transferDescriptor, validIdentity, type ProjectTransfer, type TransferCommand } from '../lib/transfer-contract';
import { formatApiError, useAsyncData } from '../lib/useAsyncData';

function saveFile(name: string, value: string) {
  const url = URL.createObjectURL(new Blob([value], { type: 'text/plain' }));
  const a = document.createElement('a'); a.href = url; a.download = name; document.body.append(a); a.click(); a.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 10000);
}
type Action = TransferCommand['action'] | 'upload_copy' | 'upload_final' | 'tail';
function isBusy(row: ProjectTransfer) { return row.jobs.some(job => job.state === 'queued' || job.state === 'running'); }
export default function Transfers() {
  const t = useT(), { me, projects, reloadProjects } = useShell(), owner = me.role === 'superadmin';
  const info = useAsyncData(async () => ({ instance: await api.instanceInfo(), system: await api.systemInfo() }), []);
  const list = useAsyncData(() => owner ? api.transfers() : Promise.resolve({ items: [] as ProjectTransfer[], configured: false, uploadLimit: 0 }), [owner]);
  const [create, setCreate] = useState<'source' | 'target' | null>(null);
  const [operation, setOperation] = useState<{ row: ProjectTransfer; action: Action } | null>(null), [error, setError] = useState<string | null>(null);
  const polling = owner && !!list.data?.items.some(row => !['completed', 'cancelled'].includes(row.state));
  useEffect(() => { if (!polling) return; const id = window.setInterval(list.reload, 2500); return () => window.clearInterval(id); }, [polling, list.reload]);
  const instance = info.data?.instance, enabled = owner && !!info.data && canUse(info.data.system, 'cloud.migrate') && instance?.operatingMode === 'normal';
  const names = new Map(projects.map(project => [project.id, project.name]));
  async function cancelJob(row: ProjectTransfer, job: string) { try { await api.cancelTransferJob(row.id, job); setError(null); list.reload(); } catch (failure) { setError(formatApiError(failure)); } }
  function refreshed() { list.reload(); reloadProjects(); }
  return <div className="host-page production-page host-stack">
    <Heading title={t('transfer.title')} description={t('transfer.description')} action={<Button onClick={() => { info.reload(); refreshed(); }}>{t('instance.refresh')}</Button>}/>
    {(info.error || list.error || error) && <ErrorBanner reason={info.error || list.error || error}>{t('instance.actionFailed')}</ErrorBanner>}
    {info.loading && !instance && <p role="status">{t('shell.loading')}</p>}
    {!owner && <p>{t('transfer.ownerOnly')}</p>}
    {owner && instance && <>
      <div className="host-columns"><Surface title={t('transfer.prepare')}><p className="host-copy">{t('transfer.prepareHint')}</p><dl className="host-facts"><div><dt>{t('instance.id')}</dt><dd>{instance.id}</dd></div><div><dt>{t('lifecycle.generation')}</dt><dd>{instance.generation}</dd></div></dl><div className="host-actions"><Button onClick={() => saveFile(`sentori-${instance.id}-identity.json`, JSON.stringify({ id: instance.id, generation: instance.generation }, null, 2))}><Download size={16}/>{t('transfer.saveIdentity')}</Button><Button disabled={!enabled} onClick={() => setCreate('target')}>{t('transfer.receive')}</Button>{info.data?.system.deployment === 'self-hosted' && <Button variant="primary" disabled={!enabled || !projects.length} onClick={() => setCreate('source')}>{t('transfer.start')}</Button>}</div>{!list.data?.configured && <p className="field-note">{t('lifecycle.configureHint')}</p>}</Surface>
      <Surface title={t('transfer.scope')} appearance="tonal"><p className="host-copy">{t('transfer.scopeHint')}</p><p className="host-copy">{t('transfer.limit')}</p><Link className="text-link" to="/docs/self-hosting">{t('lifecycle.runbook')}</Link></Surface></div>
      {list.data && !list.data.items.length && <p>{t('transfer.empty')}</p>}
      {list.data?.items.map(row => {
        const busy = isBusy(row), source = row.direction === 'source', evidence = row.projectIds.every(id => !!row.observedEvents[id]);
        const allow = enabled && row.instanceId === instance.id && row.generation === instance.generation && !busy;
        const button = (action: Action, disabled = false) => <Button key={action} small variant={action === 'seal' ? 'danger' : 'secondary'} disabled={!allow || disabled} onClick={() => setOperation({ row, action })}>{t(`transfer.action.${action}`)}</Button>;
        return <Surface key={row.id} title={t(source ? 'transfer.source' : 'transfer.target')} action={<Badge tone={row.state === 'completed' ? 'success' : row.state === 'attention' ? 'danger' : 'neutral'}>{t(`transfer.state.${row.state}`)}</Badge>}>
          <div className="host-stack" data-transfer-id={row.id}>
            <dl className="host-facts"><div><dt>{t('transfer.id')}</dt><dd>{row.id}</dd></div><div><dt>{t('transfer.source')}</dt><dd>{row.sourceInstanceId}</dd></div><div><dt>{t('transfer.target')}</dt><dd>{row.targetInstanceId}</dd></div><div><dt>{t('nav.projects')}</dt><dd>{row.projectIds.map(id => names.get(id) ?? names.get(row.projectMapping[id]) ?? id).join(', ')}</dd></div></dl>
            <ol className="host-steps">{(['copy', 'cutover', 'evidence'] as const).map((step, index) => <li key={step} data-active={index === (['copying','copied'].includes(row.state) ? 0 : ['sealed','ready'].includes(row.state) ? 1 : 2)}><span>{index + 1}</span>{t(`transfer.step.${step}`)}</li>)}</ol>
            <div><p className="host-copy">{t(source ? 'transfer.sourceHint' : 'transfer.targetHint')}</p><div className="host-actions">
              <Button small onClick={() => saveFile(`sentori-${row.id}-transfer.json`, JSON.stringify(transferDescriptor(row), null, 2))}>{t('transfer.saveDescriptor')}</Button>
              {source && ['copying','copied'].includes(row.state) && button('export_copy')}
              {source && row.state === 'copied' && button('seal')}
              {source && row.state === 'sealed' && button('export_final')}
              {source && ['sealed','observing'].includes(row.state) && button('tail')}
              {!source && ['copying','copied'].includes(row.state) && button('upload_copy')}
              {!source && row.state === 'copied' && button('upload_final')}
              {!source && row.state === 'ready' && button('activate')}
              {!source && ['active','observing'].includes(row.state) && button('verify', !evidence)}
              {!source && row.state === 'observing' && button('complete')}
              {['copying','copied'].includes(row.state) && button('cancel')}
            </div></div>
            {!source && ['ready','active','observing','completed'].includes(row.state) && <div><p className="host-copy">{t('transfer.sdkHint')}</p><div className="host-actions">{Object.entries(row.projectMapping).map(([from, to]) => <Link key={from} className="text-link" to={`/setup?project=${encodeURIComponent(to)}`}>{names.get(to) ?? to}</Link>)}</div></div>}
            {!source && ['active','observing','completed'].includes(row.state) && <div><p className="host-copy">{t('transfer.evidenceHint')}</p><dl className="host-facts">{row.projectIds.map(id => <div key={id}><dt>{names.get(row.projectMapping[id]) ?? id}</dt><dd>{row.observedEvents[id] ?? t('transfer.waitingEvent')}</dd></div>)}</dl></div>}
            {!!row.jobs.length && <ul className="host-backup-list">{row.jobs.map(job => <li key={job.id}><div className="host-backup-summary"><div><strong>{t(job.phase === 'copy' ? 'transfer.copyPackage' : 'transfer.finalPackage')}</strong><span>{job.id}</span><TimeAgo iso={job.createdAt}/></div><Badge tone={job.state === 'ready' ? 'success' : job.state === 'failed' ? 'danger' : 'neutral'}>{job.cancelRequested && job.state === 'running' ? t('lifecycle.cancelling') : t(`lifecycle.state.${job.state}`)}</Badge></div>{job.sha256 && <p className="field-note break-all">SHA-256: {job.sha256}</p>}{job.errorCode && <ErrorBanner reason={job.errorCode}>{t('lifecycle.operationFailed')}</ErrorBanner>}<div className="host-actions">{job.state === 'ready' && job.kind === 'export' && <a className="button button-secondary" download href={api.transferDownloadUrl(row.id, job.id)}>{t('lifecycle.download')}</a>}{['queued','running'].includes(job.state) && <Button small disabled={job.cancelRequested} onClick={() => void cancelJob(row, job.id)}>{t('team.cancel')}</Button>}</div></li>)}</ul>}
            {!!row.routes.length && <div><h3>{t('transfer.action.tail')}</h3><ul className="host-backup-list">{row.routes.map(route => <li key={route.projectId}><dl className="host-facts"><div><dt>{t('nav.projects')}</dt><dd>{names.get(route.projectId) ?? route.projectId}</dd></div><div><dt>{t('instance.origin')}</dt><dd>{route.origin}</dd></div><div><dt>{t('transfer.forwarded')}</dt><dd>{route.forwarded}</dd></div><div><dt>{t('transfer.failed')}</dt><dd>{route.failed}</dd></div><div><dt>{t('transfer.expires')}</dt><dd>{formatAbsolute(route.expiresAt)}</dd></div><div><dt>{t('transfer.lastSeen')}</dt><dd>{route.lastSeenAt ? formatAbsolute(route.lastSeenAt) : t('transfer.noTraffic')}</dd></div></dl>{route.lastError && <ErrorBanner reason={route.lastError}>{t('instance.actionFailed')}</ErrorBanner>}</li>)}</ul></div>}
            {row.rollbackPlan && <div><h3>{t('transfer.rollback')}</h3><p className="host-copy whitespace-pre-wrap break-words">{row.rollbackPlan}</p></div>}
          </div>
        </Surface>;
      })}
      {create && <CreateTransfer direction={create} instance={instance} onClose={() => setCreate(null)} onDone={() => { setCreate(null); refreshed(); }}/>}
      {operation && <TransferOperation {...operation} instance={instance} onClose={() => setOperation(null)} onDone={() => { setOperation(null); refreshed(); }}/>}
    </>}
  </div>;
}

// The exact request stays in memory after a lost response, including its command
// id, version, and File. A retry cannot silently become a different mutation.
function useTransferSubmit(onDone: () => void) {
  const [busy, setBusy] = useState(false), [uncertain, setUncertain] = useState(false), [error, setError] = useState<string | null>(null);
  const pending = useRef<(() => Promise<unknown>) | null>(null), locked = useRef(false);
  async function submit(make: () => () => Promise<unknown>) {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError(null);
    try { pending.current ??= make(); await pending.current(); onDone(); }
    catch (failure) {
      const lost = !(failure instanceof ApiError && failure.status >= 400 && failure.status < 500);
      setUncertain(lost); if (!lost) pending.current = null;
      setError(formatApiError(failure));
    } finally { locked.current = false; setBusy(false); }
  }
  return { busy, uncertain, error, setError, submit, frozen: busy || uncertain };
}
function CreateTransfer({ direction, instance, onClose, onDone }: { direction: 'source' | 'target'; instance: InstanceInfo; onClose: () => void; onDone: () => void }) {
  const t = useT(), { projects } = useShell(), source = direction === 'source', state = useTransferSubmit(onDone);
  const [text, setText] = useState(''), [selected, setSelected] = useState<string[]>([]);
  let target: { id: string; generation: number } | null = null;
  try { const parsed: unknown = JSON.parse(text); if (validIdentity(parsed)) target = parsed; } catch { /* Incomplete user input. */ }
  const descriptor = source ? null : parseTransferDescriptor(text);
  const valid = source ? target && target.id !== instance.id && selected.length > 0 : descriptor && descriptor.targetInstanceId === instance.id && descriptor.targetGeneration === instance.generation;
  async function read(file?: File) { if (!file) return; if (file.size > 128 * 1024) { state.setError(t('transfer.invalidDescriptor')); return; } try { setText(await file.text()); } catch (failure) { state.setError(formatApiError(failure)); } }
  function submit() { void state.submit(() => {
    if (!valid) throw new Error(t('transfer.invalidDescriptor'));
    const base = source ? { id: crypto.randomUUID(), sourceInstanceId: instance.id, sourceGeneration: instance.generation, targetInstanceId: target!.id, targetGeneration: target!.generation, projectIds: selected } : descriptor!;
    const command = { ...base, commandId: crypto.randomUUID(), direction, instanceId: instance.id, generation: instance.generation };
    return () => api.createTransfer(command);
  }); }
  return <Dialog open title={t(source ? 'transfer.start' : 'transfer.receive')} closeLabel={t('common.close')} onClose={() => { if (!state.busy) onClose(); }} footer={<><Button disabled={state.busy} onClick={onClose}>{t('common.close')}</Button><Button variant="primary" busy={state.busy} disabled={!valid} onClick={submit}>{t(state.uncertain ? 'common.retry' : source ? 'transfer.start' : 'transfer.receive')}</Button></>}>
    <div className="host-stack"><p className="host-copy">{t(source ? 'transfer.sourceCreateHint' : 'transfer.targetCreateHint')}</p><Field label={t(source ? 'transfer.identityFile' : 'transfer.descriptorFile')}><Input type="file" accept=".json,application/json" disabled={state.frozen} onChange={event => void read(event.target.files?.[0])}/></Field><Field label={t(source ? 'transfer.identityContent' : 'transfer.descriptorContent')}><Textarea value={text} disabled={state.frozen} onChange={event => setText(event.target.value)}/></Field>
      {text && !valid && <p role="status">{t('transfer.invalidDescriptor')}</p>}
      {source && <fieldset disabled={state.frozen}><legend>{t('nav.projects')}</legend><div className="host-stack">{projects.map(project => <Checkbox key={project.id} checked={selected.includes(project.id)} onChange={nextChecked => setSelected(nextChecked ? [...selected, project.id] : selected.filter(id => id !== project.id))} label={project.name}/>)}</div></fieldset>}
      {state.uncertain && <p role="status">{t('lifecycle.uncertain')}</p>}{state.error && <ErrorBanner reason={state.error}>{t('instance.actionFailed')}</ErrorBanner>}
    </div>
  </Dialog>;
}

function TransferOperation({ row, action, instance, onClose, onDone }: { row: ProjectTransfer; action: Action; instance: InstanceInfo; onClose: () => void; onDone: () => void }) {
  const t = useT(), state = useTransferSubmit(onDone);
  const exporting = action === 'export_copy' || action === 'export_final', uploading = action === 'upload_copy' || action === 'upload_final';
  const [generated] = useState(() => Array.from(crypto.getRandomValues(new Uint8Array(32)), v => v.toString(16).padStart(2, '0')).join(''));
  const [key, setKey] = useState(exporting ? generated : ''), [saved, setSaved] = useState(false), [file, setFile] = useState<File | null>(null);
  const [plan, setPlan] = useState(row.rollbackPlan ?? ''), [ack, setAck] = useState(false);
  const [projectId, setProjectId] = useState(row.projectIds[0]), [origin, setOrigin] = useState(''), [token, setToken] = useState(''), [days, setDays] = useState(30);
  const keyRead = useRef(0);
  async function readKey(file?: File) {
    const ticket = ++keyRead.current; setKey(''); setSaved(false);
    if (!file) return;
    try { const value = file.size <= 1024 ? (await file.text()).trim() : ''; if (!/^[a-f0-9]{64}$/i.test(value)) throw new Error(t('lifecycle.invalidKey')); if (ticket === keyRead.current) { setKey(value); setSaved(true); } }
    catch (failure) { if (ticket === keyRead.current) state.setError(formatApiError(failure)); }
  }
  let publicOrigin = false;
  try { const url = new URL(origin); publicOrigin = url.protocol === 'https:' && !url.username && !url.password && !url.search && !url.hash && url.pathname === '/'; } catch { /* Server also pins and validates DNS. */ }
  const valid = exporting ? /^[a-f0-9]{64}$/i.test(key) && saved : uploading ? key && file && file.size <= 1024 ** 3 : action === 'seal' || action === 'activate' ? new TextEncoder().encode(plan.trim()).length >= 30 && new TextEncoder().encode(plan.trim()).length <= 8000 && ack : action === 'complete' || action === 'cancel' ? ack : action === 'tail' ? publicOrigin && token.startsWith('st_') && Number.isInteger(days) && days >= 1 && days <= 90 : true;
  function submit() { void state.submit(() => {
    if (!valid) throw new Error(t('transfer.incomplete'));
    const identity = { commandId: crypto.randomUUID(), instanceId: instance.id, generation: instance.generation };
    if (uploading) { const upload = { command: { ...identity, recoveryKey: key }, file: file!, phase: action === 'upload_copy' ? 'copy' as const : 'final' as const, version: row.version }; return () => api.uploadTransfer(row.id, upload); }
    if (action === 'tail') { const command = { ...identity, version: row.version, projectId, targetOrigin: new URL(origin).origin, targetToken: token, expiresInDays: days }; return () => api.transferTail(row.id, command); }
    const command: TransferCommand = { ...identity, version: row.version, action, ...(exporting ? { recoveryKey: key } : {}), ...(['seal','activate'].includes(action) ? { rollbackPlan: plan } : {}), ...(action === 'verify' ? { events: row.observedEvents } : {}), ...(action === 'complete' ? { sdkUpdated: ack } : {}) };
    return () => api.transferCommand(row.id, command);
  }); }
  return <Dialog open title={t(`transfer.action.${action}`)} closeLabel={t('common.close')} onClose={() => { if (!state.busy) onClose(); }} footer={<><Button disabled={state.busy} onClick={onClose}>{t('common.close')}</Button><Button variant={action === 'seal' ? 'danger' : 'primary'} busy={state.busy} disabled={!valid} onClick={submit}>{t(state.uncertain ? 'common.retry' : `transfer.action.${action}`)}</Button></>}>
    <div className="host-stack">
      {(exporting || uploading) && <><p className="host-copy">{t('transfer.packageHint')}</p>{exporting && <><Button disabled={state.frozen || !key} onClick={() => saveFile(`sentori-${row.id}-${action}-recovery.key`, key + '\n')}><Download size={16}/>{t('lifecycle.saveKey')}</Button><Checkbox disabled={state.frozen} checked={saved} onChange={nextChecked => setSaved(nextChecked)} label={t('lifecycle.savedKey')}/></>}<Field label={t('lifecycle.keyFile')}><Input type="file" accept=".key,text/plain" disabled={state.frozen} onChange={event => void readKey(event.target.files?.[0])}/></Field></>}
      {uploading && <><Field label={t('lifecycle.archiveFile')}><Input type="file" accept=".sentori" disabled={state.frozen} onChange={event => setFile(event.target.files?.[0] ?? null)}/></Field><p className="host-copy">{t('transfer.limit')}</p>{file && file.size > 1024 ** 3 && <p role="alert">{t('lifecycle.uploadTooLarge')}</p>}</>}
      {['seal','activate'].includes(action) && <><p className="host-copy">{t(action === 'seal' ? 'transfer.sealHint' : 'transfer.activateHint')}</p><Field label={t('transfer.rollback')} hint={t('transfer.rollbackHint')}><Textarea maxLength={8000} disabled={state.frozen} value={plan} onChange={event => setPlan(event.target.value)}/></Field><Checkbox disabled={state.frozen} checked={ack} onChange={nextChecked => setAck(nextChecked)} label={t('transfer.cutoverAck')}/></>}
      {action === 'verify' && <p className="host-copy">{t('transfer.evidenceHint')}</p>}
      {(action === 'complete' || action === 'cancel') && <Checkbox disabled={state.frozen} checked={ack} onChange={nextChecked => setAck(nextChecked)} label={t(action === 'complete' ? 'transfer.sdkAck' : 'transfer.cancelAck')}/>}
      {action === 'tail' && <><p className="host-copy">{t('transfer.tailHint')}</p><fieldset disabled={state.frozen}><legend>{t('nav.projects')}</legend>{row.projectIds.map(id => <Radio key={id} name="tail-project" value={id} checked={projectId === id} onChange={() => setProjectId(id)} label={id}/>)}</fieldset><Field label={t('transfer.targetOrigin')}><Input type="url" value={origin} disabled={state.frozen} onChange={event => setOrigin(event.target.value)}/></Field><Field label={t('transfer.targetToken')}><Input type="password" autoComplete="off" value={token} disabled={state.frozen} onChange={event => setToken(event.target.value)}/></Field><Field label={t('transfer.days')}><Input type="number" min={1} max={90} value={days} disabled={state.frozen} onChange={event => setDays(Number(event.target.value))}/></Field></>}
      {state.uncertain && <p role="status">{t('lifecycle.uncertain')}</p>}{state.error && <ErrorBanner reason={state.error}>{t('instance.actionFailed')}</ErrorBanner>}
    </div>
  </Dialog>;
}
