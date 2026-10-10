import { CloudWorkspace } from './CloudWorkspace';
import { InstanceHealthPanel } from './InstanceHealth';
import { useRef, useState } from 'react';

import { Button, ErrorBanner, Field, Input, Panel, TimeAgo } from '../components/ui';
import { useT } from '../i18n';
import { api, ApiError } from '../lib/api';
import { canUse, type InstanceInfo, type SystemInfo } from '../lib/system-contract';
import { formatApiError, useAsyncData } from '../lib/useAsyncData';

export default function InstanceSettings() {
  const t = useT();
  const { data, error, loading, reload, setData } = useAsyncData(async () => {
    const [instance, system] = await Promise.all([api.instanceInfo(), api.systemInfo()]);
    return { instance, system };
  }, []);
  if (data?.system.deployment === 'cloud') return <CloudWorkspace owner={canUse(data.system, 'members.assign')} />;
  return <div className="space-y-4">
    {error && <ErrorBanner reason={error}>{t('instance.loadFailed')}</ErrorBanner>}
    {loading && <p role="status">{t('shell.loading')}</p>}
    {data && <InstanceForm key={data.instance.id} {...data}
      onChange={instance => setData(previous => previous ? { ...previous, instance } : null)} />}
    <Button disabled={loading} onClick={reload}>{t('instance.refresh')}</Button>
  </div>;
}

function InstanceForm({ instance, system, onChange }: {
  instance: InstanceInfo;
  system: SystemInfo;
  onChange: (value: InstanceInfo) => void;
}) {
  const t = useT();
  const [name, setName] = useState(instance.name);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState('');
  const [previousInstance, setPreviousInstance] = useState(instance);
  if (previousInstance !== instance) {
    setPreviousInstance(instance);
    setName(instance.name);
  }
  const inFlight = useRef(false);
  const allowed = canUse(system, 'instance.configure');
  const dirty = name.trim() !== instance.name;
  const proof = instance.lastCheck;
  const passed = proof && proof.version === instance.version && proof.origin === instance.origin
    && proof.database && proof.blobStorage && proof.durableStorage && proof.originValid;

  async function run(action: 'save' | 'check' | 'initialize') {
    if (inFlight.current) return;
    inFlight.current = true;
    setBusy(true); setError(null); setNotice('');
    try {
      if (action === 'save') onChange(await api.updateInstance(name.trim(), instance.version));
      else if (action === 'initialize') onChange(await api.initializeInstance(instance.version));
      else { await api.checkInstance(); onChange(await api.instanceInfo()); }
      setNotice(t(action === 'save' ? 'settings.saved' : action === 'check' ? 'instance.checked' : 'instance.ready'));
    } catch (failure) {
      const message = failure instanceof ApiError && failure.code === 'instance_changed'
        ? t('instance.conflict')
        : failure instanceof ApiError && failure.code === 'environment_check_required'
          ? t('instance.checkRequired') : formatApiError(failure);
      setError(message);
    } finally { inFlight.current = false; setBusy(false); }
  }

  return <div className="space-y-4" aria-busy={busy}>
    {canUse(system, 'instance.healthcheck') && <InstanceHealthPanel />}
    {instance.operatingMode === 'restore_validation' && <Panel title={t('instance.recoveryValidation')} padded>
      <p className="text-sm text-fg-muted">{t('instance.recoveryHint')}</p>
    </Panel>}
    {error && <ErrorBanner reason={error}>{t('instance.actionFailed')}</ErrorBanner>}
    {notice && <p role="status">{notice}</p>}
    <Panel title={t('instance.identity')} padded>
      <div className="space-y-3">
        <Field label={t('instance.name')}><Input value={name} maxLength={80} disabled={!allowed || busy}
          onChange={event => setName(event.target.value)} /></Field>
        <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2 text-sm">
          <dt>{t('instance.id')}</dt><dd className="break-all font-mono">{instance.id}</dd>
          <dt>{t('instance.origin')}</dt><dd className="break-all">{instance.origin}</dd>
          <dt>{t('instance.version')}</dt><dd>{instance.serverVersion}</dd>
          <dt>{t('instance.status')}</dt><dd>{t(instance.initializedAt ? 'instance.ready' : 'instance.pending')}</dd>
        </dl>
        <p className="text-xs text-fg-subtle">{t('instance.environment')}</p>
        {allowed && <Button disabled={busy || !dirty || !name.trim()} onClick={() => void run('save')}>{t('settings.save')}</Button>}
      </div>
    </Panel>
    <Panel title={t('instance.checks')} padded>
      <div className="space-y-3">
        <p className="text-sm text-fg-muted">{t('instance.checkHint')}</p>
        <dl className="grid grid-cols-[minmax(0,1fr)_auto] gap-2 text-sm">
          {(['database', 'blobStorage', 'durableStorage', 'originValid'] as const).map(check =>
            <div key={check} className="contents"><dt>{t(`instance.check.${check}`)}</dt>
              <dd>{t(!proof ? 'instance.unchecked' : proof[check] ? 'instance.passed' : 'instance.failed')}</dd></div>)}
        </dl>
        {instance.checkedAt && <p className="text-xs text-fg-subtle">{t('instance.lastCheck')} <TimeAgo iso={instance.checkedAt} /></p>}
        {allowed && <div className="flex flex-wrap gap-2">
          <Button disabled={busy || dirty} onClick={() => void run('check')}>{t('instance.runChecks')}</Button>
          {!instance.initializedAt && <Button variant="primary" disabled={busy || dirty || !passed}
            onClick={() => void run('initialize')}>{t('instance.initialize')}</Button>}
        </div>}
      </div>
    </Panel>
  </div>;
}
