import { useRef, useState } from 'react';
import { Link } from 'react-router-dom';
import { Button, Field, Input } from '../../shared/design/controls';
import { Dialog } from '../../shared/design/Dialog';
import { ErrorBanner } from '../components/ui';
import { useLocale, useT } from '../i18n';
import { api, ApiError } from '../lib/api';
import type { InstanceInfo, ResetPermit } from '../lib/system-contract';

const quote = (value: string) => `'${value.replaceAll("'", "'\\''")}'`;

export default function ResetInstance({ instance, onClose }: { instance: InstanceInfo; onClose: () => void }) {
  const t = useT(), [name, setName] = useState(''), [identity, setIdentity] = useState(''), [generation, setGeneration] = useState('');
  const locale = useLocale();
  const [password, setPassword] = useState(''), [acknowledged, setAcknowledged] = useState(false), [busy, setBusy] = useState(false);
  const [permit, setPermit] = useState<ResetPermit | null>(null), [error, setError] = useState<string | null>(null);
  const locked = useRef(false);
  const valid = name === instance.name && identity === instance.id && generation === String(instance.generation) && password.length > 0 && acknowledged;
  async function prepare() {
    if (!valid || locked.current) return;
    locked.current = true; setBusy(true); setError(null);
    try {
      setPermit(await api.prepareReset({ instanceId: identity, instanceName: name, generation: Number(generation), currentPassword: password }));
    } catch (failure) {
      setError(t(failure instanceof ApiError && failure.code === 'wrong_password' ? 'reset.wrongPassword' : failure instanceof ApiError && failure.code === 'instance_changed' ? 'lifecycle.reason.instance_changed' : 'reset.failed'));
    } finally { setPassword(''); locked.current = false; setBusy(false); }
  }
  function downloadPermit() {
    if (!permit) return;
    const url = URL.createObjectURL(new Blob([permit.permit + '\n'], { type: 'text/plain' }));
    const anchor = document.createElement('a'); anchor.href = url; anchor.download = `sentori-reset-${permit.operationId}.permit`;
    document.body.append(anchor); anchor.click(); anchor.remove(); window.setTimeout(() => URL.revokeObjectURL(url), 10000);
  }
  const command = permit ? `sentori-operator reset \\\n  --permit-file ${quote(`sentori-reset-${permit.operationId}.permit`)} \\\n  --instance-id ${quote(instance.id)} --generation ${instance.generation} \\\n  --instance-name ${quote(instance.name)} --blob-dir /data/blobs \\\n  --backup /recovery/reset-${permit.operationId}.sentori \\\n  --journal /recovery/reset-${permit.operationId}.json \\\n  --key-file /keys/recovery.key --confirm-independent-storage` : '';
  return <Dialog open title={t('reset.title')} closeLabel={t('common.close')} onClose={() => { if (!busy) onClose(); }} footer={<><Button disabled={busy} onClick={onClose}>{t('common.close')}</Button>{!permit && <Button variant="danger" busy={busy} disabled={!valid} onClick={() => void prepare()}>{t('reset.prepare')}</Button>}</>}>
    <div className="host-stack">
      {permit ? <>
        <p role="status">{t('reset.prepared')}</p><p>{t('reset.expires')} <time dateTime={permit.expiresAt} title={permit.expiresAt}>{new Date(permit.expiresAt).toLocaleString(locale)}</time></p>
        <Button onClick={downloadPermit}>{t('reset.downloadPermit')}</Button>
        <ol className="host-copy list-decimal pl-5 space-y-2"><li>{t('reset.storageStep')}</li><li>{t('reset.stopStep')}</li><li>{t('reset.executeStep')}</li><li>{t('reset.restartStep')}</li></ol>
        <pre className="overflow-x-auto text-xs p-3 bg-canvas-subtle rounded">{command}</pre>
        <p className="host-copy">{t('reset.interrupted')}</p>
        <pre className="overflow-x-auto text-xs p-3 bg-canvas-subtle rounded">{`sentori-operator finish-reset --journal /recovery/reset-${permit.operationId}.json --key-file /keys/recovery.key`}</pre>
      </> : <>
        <p className="host-copy">{t('reset.consequences')}</p>
        <dl className="host-facts">{[[t('instance.name'), instance.name], [t('instance.id'), instance.id], [t('lifecycle.generation'), String(instance.generation)]].map(([label, value]) => <div key={label}><dt>{label}</dt><dd>{value}</dd></div>)}</dl>
        <Field label={t('reset.typeName')}><Input autoComplete="off" value={name} onChange={event => setName(event.target.value)}/></Field>
        <Field label={t('reset.typeId')}><Input autoComplete="off" value={identity} onChange={event => setIdentity(event.target.value)}/></Field>
        <Field label={t('reset.typeGeneration')}><Input autoComplete="off" inputMode="numeric" value={generation} onChange={event => setGeneration(event.target.value)}/></Field>
        <Field label={t('reset.password')}><Input type="password" autoComplete="current-password" value={password} onChange={event => setPassword(event.target.value)}/></Field>
        <label className="choice-control"><input type="checkbox" checked={acknowledged} onChange={event => setAcknowledged(event.target.checked)}/><span className="choice-label">{t('reset.acknowledge')}</span></label>
      </>}
      {error && <ErrorBanner reason={error}>{t('instance.actionFailed')}</ErrorBanner>}
      <Link className="text-link" to="/docs/self-hosting">{t('lifecycle.runbook')}</Link>
    </div>
  </Dialog>;
}
