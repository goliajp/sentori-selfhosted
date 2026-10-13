import { Checkbox } from "../../shared/design/Checkbox";
import { useEffect, useRef, useState } from 'react';
import { Link, Navigate, useSearchParams } from 'react-router-dom';
import { Database, Download, HardDrive, Server, ShieldCheck } from 'lucide-react';
import { Heading, Surface } from '../../shared/design/Surface';
import { Badge, Button, Field, Input } from '../../shared/design/controls';
import { Metric } from '../../shared/design/Metric';
import { Tabs } from '../../shared/design/Tabs';
import { Dialog } from '../../shared/design/Dialog';
import { ErrorBanner, TimeAgo } from '../components/ui';
import { useShell } from '../App';
import { useT } from '../i18n';
import { api, ApiError } from '../lib/api';
import { canUse, type BackupCommand, type InstanceInfo, type InstanceOperation } from '../lib/system-contract';
import { formatApiError, useAsyncData } from '../lib/useAsyncData';
import InstanceSettings from './Instance';
import ResetInstance from './ResetInstance';

const reasonCodes = ['cancelled', 'instance_changed', 'instance_restored', 'key_unavailable', 'capture_failed', 'verification_failed', 'retry_exhausted'] as const;
function operationReason(code: string) {
  const known = reasonCodes.find(value => value === code);
  return known ? `lifecycle.reason.${known}` as const : 'lifecycle.operationFailed';
}
const sections = ['overview', 'setup', 'backups', 'activity'] as const;
export default function SelfHosted() {
  const t = useT(), { me, projects } = useShell(), owner = me.role === 'superadmin';
  const [params, setParams] = useSearchParams(), requested = params.get('tab');
  const tab = sections.find(value => value === requested) ?? 'overview';
  const setTab = (value: string) => { const next = new URLSearchParams(params); next.set('tab', value); setParams(next); };
  const info = useAsyncData(async () => ({ instance: await api.instanceInfo(), system: await api.systemInfo() }), []);
  const canBackup = owner && info.data?.system.deployment === 'self-hosted';
  const list = useAsyncData(() => canBackup ? api.instanceOperations() : Promise.resolve({ items: [] as InstanceOperation[], configured: false }), [canBackup]);
  const [create, setCreate] = useState(false), [upload, setUpload] = useState(false), [failure, setFailure] = useState<string | null>(null);
  const [reset, setReset] = useState(false);
  const active = !!list.data?.items.some(row => row.state === 'queued' || row.state === 'running');
  useEffect(() => { if (!active) return; const timer = window.setInterval(list.reload, 2000); return () => window.clearInterval(timer); }, [active, list.reload]);
  const latest = list.data?.items.find(row => row.state === 'ready' && row.manifest?.sourceInstanceId === info.data?.instance.id && row.manifest?.generation === info.data?.instance.generation);
  const instance = info.data?.instance;
  async function cancel(id: string) { try { await api.cancelBackup(id); setFailure(null); list.reload(); } catch (error) { setFailure(formatApiError(error)); } }
  if (info.data?.system.deployment === 'cloud') return <Navigate to="/settings/instance" replace/>;
  return <div className="host-page production-page">
    <Heading title={t('lifecycle.title')} description={t('lifecycle.description')} action={<Button onClick={() => { info.reload(); list.reload(); }}>{t('instance.refresh')}</Button>}/>
    {(info.error || list.error || failure) && <ErrorBanner reason={info.error || list.error || failure}>{t('instance.actionFailed')}</ErrorBanner>}
    {info.loading && !instance && <p role="status">{t('shell.loading')}</p>}
    {!owner && <p className="host-copy">{t('lifecycle.ownerOnly')}</p>}
    {instance && <>
      {instance.operatingMode === 'restore_validation' && <Surface title={t('instance.recoveryValidation')}><p>{t('instance.recoveryHint')}</p></Surface>}
      <Tabs label={t('lifecycle.title')} value={tab} onChange={setTab} variant="line" items={sections.map(section => [section, t(`lifecycle.${section}`)])}>
        {tab === 'overview' && <div className="host-stack">
          <Surface title={t('transfer.title')}><p className="host-copy">{t('transfer.description')}</p><Link className="text-link" to="/transfers">{t('transfer.title')}</Link></Surface>
          <div className="host-metrics"><Metric label={t('nav.projects')} value={projects.length}/><Metric label={t('instance.version')} value={instance.serverVersion}/><Metric label={t('lifecycle.latestBackup')} value={latest ? <TimeAgo iso={latest.finishedAt ?? latest.createdAt}/> : t('lifecycle.noBackup')}/></div>
          <div className="host-columns"><Surface title={t('instance.identity')} action={<Server size={20}/>}><dl className="host-facts">{[[t('instance.name'), instance.name], [t('instance.id'), instance.id], [t('instance.origin'), instance.origin], [t('lifecycle.generation'), String(instance.generation)]].map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl><div className="host-actions"><Button onClick={() => setTab('setup')}>{t('lifecycle.setup')}</Button><Button onClick={() => setTab('backups')}>{t('lifecycle.backups')}</Button></div></Surface>
          <Surface title={t('lifecycle.recovery')} appearance="tonal"><p className="host-copy">{t('lifecycle.offInstance')}</p><p className="host-copy">{t('lifecycle.recoveryHint')}</p><Link className="text-link" to="/docs/self-hosting">{t('lifecycle.runbook')}</Link><div className="host-recovery-row"><div><h3>{t('reset.title')}</h3><p>{t('reset.hint')}</p></div><Button variant="danger" disabled={!owner || active || !canUse(info.data!.system, 'instance.reset') || instance.operatingMode !== 'normal'} onClick={() => setReset(true)}>{t('reset.title')}</Button></div></Surface></div>
        </div>}
        {tab === 'setup' && <InstanceSettings/>}
        {tab === 'backups' && <div className="host-stack"><Surface title={t('lifecycle.backups')} action={owner && <Button variant="primary" disabled={active || !canUse(info.data!.system, 'instance.backup') || instance.operatingMode !== 'normal'} onClick={() => setCreate(true)}><Download size={16}/>{t('lifecycle.create')}</Button>}>
          <p className="host-copy">{t('lifecycle.backupHint')}</p><div className="host-includes"><span><Database size={16}/>{t('lifecycle.database')}</span><span><HardDrive size={16}/>{t('lifecycle.files')}</span><span><ShieldCheck size={16}/>{t('lifecycle.encrypted')}</span></div><p className="host-copy">{t('lifecycle.offInstance')}</p>
          {owner && <div className="host-actions"><Button disabled={active || !canUse(info.data!.system, 'instance.backup') || instance.operatingMode !== 'normal'} onClick={() => setUpload(true)}>{t('lifecycle.upload')}</Button></div>}
          {owner && list.data && !list.data.configured && <p role="status" className="host-copy">{t('lifecycle.configureHint')}</p>}
          {list.data && !list.data.items.length && <p>{t('lifecycle.noBackup')}</p>}
          <ul className="host-backup-list">{list.data?.items.map(row => <li key={row.id} data-operation-id={row.id}><div className="host-backup-summary"><div><strong>{t(row.kind === 'verify' ? 'lifecycle.uploadedArchive' : 'lifecycle.instanceBackup')}</strong><span>{row.id}</span><TimeAgo iso={row.createdAt}/><span>{t('lifecycle.generation')} {row.generation} · {row.bytes === null ? `${row.progress.rows ?? 0} ${t('lifecycle.rows')}` : `${(row.bytes / 1024 / 1024).toFixed(2)} MiB`}</span></div><Badge tone={row.state === 'ready' ? 'success' : row.state === 'failed' ? 'danger' : 'neutral'}>{row.cancelRequested && row.state === 'running' ? t('lifecycle.cancelling') : row.state === 'running' && row.progress.phase === 'verifying' ? t('lifecycle.verifying') : t(`lifecycle.state.${row.state}`)}</Badge></div>
            {row.kind === 'verify' && row.manifest && <p className="field-note break-all">{t('lifecycle.sourceInstance')}: {row.manifest.sourceInstanceId} · {t('lifecycle.generation')} {row.manifest.generation}</p>}{row.sha256 && <p className="field-note break-all">SHA-256: {row.sha256}</p>}{row.errorCode && <p className="field-note">{t(operationReason(row.errorCode))}</p>}
            <div className="host-actions">{row.state === 'ready' && <a className="button button-secondary" href={api.backupDownloadUrl(row.id)} download>{t('lifecycle.download')}</a>}{(row.state === 'queued' || row.state === 'running') && <Button small disabled={row.cancelRequested} onClick={() => void cancel(row.id)}>{t('team.cancel')}</Button>}</div>
          </li>)}</ul>
        </Surface><Surface title={t('lifecycle.recovery')}><p className="host-copy">{t('lifecycle.recoveryHint')}</p><p className="host-copy">{t('lifecycle.verifyHint')}</p><pre className="overflow-x-auto text-xs p-3 bg-canvas-subtle rounded">{'sentori-operator verify --from backup.sentori --key-file recovery.key\nsentori-operator migrate\nsentori-operator restore --from backup.sentori --key-file recovery.key --blob-dir /data/restored'}</pre><Link className="text-link" to="/docs/self-hosting">{t('lifecycle.runbook')}</Link></Surface></div>}
        {tab === 'activity' && <Surface title={t('lifecycle.activity')}><p className="host-copy">{t('lifecycle.auditHint')}</p><Link className="text-link" to="/settings/audit">{t('lifecycle.openAudit')}</Link><ol className="host-log">{list.data?.items.map(row => <li key={row.id}><TimeAgo iso={row.createdAt}/><span>{t(`lifecycle.state.${row.state}`)}</span><code className="break-all">{row.id}</code></li>)}</ol></Surface>}
      </Tabs>
      {upload && <VerifyBackup instance={instance} onClose={() => setUpload(false)} onCreated={() => { setUpload(false); list.reload(); }}/>}
      {create && <CreateBackup instance={instance} onClose={() => setCreate(false)} onCreated={() => { setCreate(false); list.reload(); }}/>}
      {reset && <ResetInstance instance={instance} onClose={() => setReset(false)}/>}
    </>}
  </div>;
}

function CreateBackup({ instance, onClose, onCreated }: { instance: InstanceInfo; onClose: () => void; onCreated: () => void }) {
  const t = useT(), [key] = useState(() => Array.from(crypto.getRandomValues(new Uint8Array(32)), value => value.toString(16).padStart(2, '0')).join(''));
  const [saved, setSaved] = useState(false), [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null), [uncertain, setUncertain] = useState(false);
  const pending = useRef<BackupCommand | null>(null), locked = useRef(false);
  function downloadKey() {
    const url = URL.createObjectURL(new Blob([key + '\n'], { type: 'text/plain' }));
    const anchor = document.createElement('a'); anchor.href = url; anchor.download = `sentori-${instance.id}-recovery.key`; document.body.append(anchor); anchor.click(); anchor.remove(); window.setTimeout(() => URL.revokeObjectURL(url), 10000);
  }
  async function submit() {
    if (locked.current || !saved) return;
    locked.current = true; setBusy(true); setError(null);
    pending.current ??= { commandId: crypto.randomUUID(), instanceId: instance.id, generation: instance.generation, recoveryKey: key };
    try { await api.createBackup(pending.current); onCreated(); }
    catch (failure) { setUncertain(!(failure instanceof ApiError && failure.status >= 400 && failure.status < 500)); setError(formatApiError(failure)); }
    finally { locked.current = false; setBusy(false); }
  }
  return <Dialog open title={t('lifecycle.create')} closeLabel={t('common.close')} onClose={() => { if (!busy) onClose(); }} footer={<><Button disabled={busy} onClick={onClose}>{t('team.cancel')}</Button><Button variant="primary" busy={busy} disabled={!saved} onClick={() => void submit()}>{t(uncertain ? 'common.retry' : 'lifecycle.create')}</Button></>}>
    <div className="host-stack"><p className="host-copy">{t('lifecycle.keyHint')}</p><Button onClick={downloadKey}><Download size={16}/>{t('lifecycle.saveKey')}</Button><Checkbox checked={saved} onChange={nextChecked => setSaved(nextChecked)} label={t('lifecycle.savedKey')}/>{uncertain && <p role="status">{t('lifecycle.uncertain')}</p>}{error && <ErrorBanner reason={error}>{t('instance.actionFailed')}</ErrorBanner>}</div>
  </Dialog>;
}

function VerifyBackup({ instance, onClose, onCreated }: { instance: InstanceInfo; onClose: () => void; onCreated: () => void }) {
  const t = useT(), [archive, setArchive] = useState<File | null>(null), [key, setKey] = useState('');
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null), [uncertain, setUncertain] = useState(false);
  const pending = useRef<BackupCommand | null>(null), locked = useRef(false), keyRead = useRef(0);
  async function readKey(file?: File) {
    const ticket = ++keyRead.current;
    setKey(''); setError(null);
    if (!file) return;
    try {
      if (file.size > 1024) throw new Error(t('lifecycle.invalidKey'));
      const value = (await file.text()).trim();
      if (!/^[0-9a-fA-F]{64}$/.test(value)) throw new Error(t('lifecycle.invalidKey'));
      if (ticket === keyRead.current) setKey(value);
    } catch (failure) { if (ticket === keyRead.current) setError(formatApiError(failure)); }
  }
  async function submit() {
    if (locked.current || !archive || !key || archive.size > 1024 ** 3) return;
    locked.current = true; setBusy(true); setError(null);
    pending.current ??= { commandId: crypto.randomUUID(), instanceId: instance.id, generation: instance.generation, recoveryKey: key };
    try { await api.verifyBackup(pending.current, archive); onCreated(); }
    catch (failure) {
      const lost = !(failure instanceof ApiError && failure.status >= 400 && failure.status < 500);
      setUncertain(lost); if (!lost) pending.current = null;
      setError(formatApiError(failure));
    } finally { locked.current = false; setBusy(false); }
  }
  return <Dialog open title={t('lifecycle.upload')} closeLabel={t('common.close')} onClose={() => { if (!busy) onClose(); }} footer={<><Button disabled={busy} onClick={onClose}>{t('team.cancel')}</Button><Button variant="primary" busy={busy} disabled={!archive || !key || archive.size > 1024 ** 3} onClick={() => void submit()}>{t(uncertain ? 'common.retry' : 'lifecycle.verify')}</Button></>}>
    <div className="host-stack"><p className="host-copy">{t('lifecycle.uploadHint')}</p>
      <Field label={t('lifecycle.archiveFile')}><Input type="file" accept=".sentori" disabled={busy || uncertain} onChange={event => { setArchive(event.target.files?.[0] ?? null); setError(null); }}/></Field>
      <Field label={t('lifecycle.keyFile')}><Input type="file" accept=".key,text/plain" disabled={busy || uncertain} onChange={event => void readKey(event.target.files?.[0])}/></Field>
      {archive && archive.size > 1024 ** 3 && <p role="alert">{t('lifecycle.uploadTooLarge')}</p>}
      {uncertain && <p role="status">{t('lifecycle.uncertain')}</p>}{error && <ErrorBanner reason={error}>{t('instance.actionFailed')}</ErrorBanner>}
    </div>
  </Dialog>;
}
