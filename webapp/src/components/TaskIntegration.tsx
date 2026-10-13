import { Checkbox } from "../../shared/design/Checkbox";
import { useEffect, useRef, useState } from 'react';
import { Surface } from '../../shared/design/Surface';
import { Button, Field, Input } from '../../shared/design/controls';
import { useShell } from '../App';
import { useT } from '../i18n';
import { api } from '../lib/api';
import { formatApiError } from '../lib/useAsyncData';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';
import { ErrorBanner } from './ui';

type Config = { repository: string; environment: string; enabled: boolean; version: number };
export function TaskIntegration({ project }: { project: string }) {
  const t = useT(), { me } = useShell();
  const state = useScopedAsyncData(project, () => api.taskIntegration(project));
  const [draft, setDraft] = useState<Config | null>(null), [rotate, setRotate] = useState(false), [secret, setSecret] = useState<string | null>(null), [error, setError] = useState<string | null>(null), [busy, setBusy] = useState(false);
  const visible = useRef(true);
  useEffect(() => { visible.current=true; return () => { visible.current=false; setSecret(null); }; }, []);
  const lock = useRef(false), current = draft ?? state.data?.configuration ?? { repository: '', environment: 'production', enabled: true, version: 0 };
  const patch = (values: Partial<Config>) => setDraft({ ...current, ...values });
  async function save() {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError(null); setSecret(null);
    try {
      const result = await api.configureTaskIntegration(project, { repository: current.repository, environment: current.environment, enabled: current.enabled, expectedVersion: current.version, rotateSecret: rotate });
      setSecret(visible.current ? result.secret : null); setDraft({ ...current, version: result.version }); setRotate(false); state.reload();
    } catch (failure) { setError(formatApiError(failure)); }
    finally { lock.current = false; setBusy(false); }
  }
  return <details className="mt-6"><summary className="text-link text-sm">{t('team.automation')}</summary><div className="mt-4"><Surface title={t('team.automation')}><div className="form-stack">
    <p className="muted text-sm">{t('team.integrationHint')}</p>
    {(error || state.error) && <ErrorBanner reason={error || state.error || ''}>{t('team.failed')}</ErrorBanner>}
    <Button small onClick={() => { setDraft(null); state.reload(); }}>{t('team.refresh')}</Button>
    {state.data && !state.data.vaultConfigured && <p>{t('team.vaultRequired')}</p>}
    {state.data && <form className="form-stack" onSubmit={e => { e.preventDefault(); void save(); }}><fieldset className="form-stack min-w-0" disabled={busy || me.role !== 'superadmin' || !state.data.vaultConfigured}>
      <div className="team-form-grid"><Field label={t('team.repository')}><Input required value={current.repository} onChange={e => patch({ repository: e.target.value })}/></Field><Field label={t('team.environment')}><Input required value={current.environment} onChange={e => patch({ environment: e.target.value })}/></Field></div>
      <Checkbox checked={current.enabled} onChange={nextChecked => patch({ enabled: nextChecked })} label={t('team.enabled')}/>
      {current.version > 0 && <Checkbox checked={rotate} onChange={nextChecked => setRotate(nextChecked)} label={t('team.rotate')}/>}
      <div><Button type="submit" variant="primary" busy={busy}>{t('team.save')}</Button></div>
    </fieldset></form>}
    {secret && <Field label={t('team.signingKey')} hint={t('team.secretOnce')}><Input readOnly value={secret}/></Field>}
    {state.data?.configuration && <><Field label={t('team.endpoint')}><Input readOnly value={`${location.origin}/hooks/tasks/${project}`}/></Field><p className="text-sm muted">{t('team.signatureHelp')}</p></>}
  </div></Surface></div></details>;
}
