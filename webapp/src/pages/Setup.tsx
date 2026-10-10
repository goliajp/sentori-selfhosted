import { Heading, Surface } from '../../shared/design/Surface';
import { Button as DesignButton } from '../../shared/design/controls';
import { FlowActions, FlowFrame, FlowSteps } from '../../shared/design/Flow';
import { ArrowRight, Check, Circle, ExternalLink } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';

import { useShell } from '../App';
import { CopyCode } from '../components/CopyCode';
import { ErrorBanner, Field, Input, Select } from '../components/ui';
import { useT } from '../i18n';
import { api, type Project } from '../lib/api';
import { platformLabel } from '../lib/platform-label';
import { findSetupEvent, hasReadableSource, SETUP_DOCS, SETUP_PLATFORMS, setupCheckId, setupPlatform, type SetupPlatform } from '../lib/setup';
import { setupSnippets } from '../lib/setup-snippets';
import { formatApiError } from '../lib/useAsyncData';

type SetupProject = Pick<Project, 'id' | 'name' | 'platform'>;

export default function Setup() {
  const t = useT();
  const { activeProject, projects, setActiveProjectId, reloadProjects, me } = useShell();
  const [params, setParams] = useSearchParams();
  const [created, setCreated] = useState<SetupProject | null>(null);
  const project = params.has('new') ? null : activeProject ?? (created?.id === params.get('project') ? created : null);
  return (
    <div className="production-page">
        <Heading title={t('setup.title')} description={t('setup.subtitle')} />
        <div className="control-bar">
          {projects.length > 0 && (
            <Select
              aria-label={t('settings.fieldProject')}
              value={project?.id ?? ''}
              onChange={(e) => { setCreated(null); setActiveProjectId(e.target.value); setParams({project:e.target.value}); }}
            >
              {!project && <option value="" disabled>{t('setup.chooseProject')}</option>}
              {projects.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}
              {created && !projects.some((p) => p.id === created.id) && <option value={created.id}>{created.name}</option>}
            </Select>
          )}
        </div>
        {!project ? (
          me.role === 'superadmin' && (params.has('new') || projects.length === 0) ? <CreateProject onCreated={(p) => {
            setCreated(p); setActiveProjectId(p.id); void reloadProjects(); setParams({project:p.id});
          }} /> : <p className="rounded-xl border border-border bg-surface p-6 text-sm text-fg-muted">{t('setup.askOwner')}</p>
        ) : <ProjectSetup key={project.id} project={project} />}
    </div>
  );
}

function CreateProject({ onCreated }: { onCreated: (p: SetupProject) => void }) {
  const t = useT();
  const [name, setName] = useState('');
  const [platform, setPlatform] = useState<SetupPlatform>('react-native');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  return (
    <Surface title={t('setup.createTitle')} className="max-w-xl"><form className="form-stack" onSubmit={(e) => {
      e.preventDefault();
      if (!name.trim() || busy) return;
      setBusy(true); setError(null);
      void api.createProject(name.trim(), platform).then(
        ({ id }) => onCreated({ id, name: name.trim(), platform }),
        (err: unknown) => { setError(formatApiError(err)); setBusy(false); },
      );
    }}>
      <Field label={t('settings.projectName')}><Input required maxLength={200} disabled={busy} autoComplete="off" value={name} onChange={(e) => setName(e.target.value)} /></Field>
      <Field label={t('setup.platform')}><Select disabled={busy} value={platform} onChange={(e) => setPlatform(setupPlatform(e.target.value))}>{SETUP_PLATFORMS.map((p) => <option key={p} value={p}>{platformLabel(p, t)}</option>)}</Select></Field>
      {error && <ErrorBanner reason={error}>{t('settings.createProjectFailed')}</ErrorBanner>}
      <DesignButton type="submit" variant="primary" busy={busy} disabled={!name.trim()}>{t(busy ? 'setup.creating' : 'settings.createProject')}<ArrowRight className="h-4 w-4" aria-hidden /></DesignButton>
    </form></Surface>
  );
}

function ProjectSetup({ project }: { project: SetupProject }) {
  const t = useT();
  const [platform, setPlatform] = useState<SetupPlatform>(() => setupPlatform(project.platform));
  const [token, setToken] = useState('');
  const [release, setRelease] = useState('myapp@1.0.0');
  const [origin, setOrigin] = useState(() => window.location.origin);
  const [checkId, setCheckId] = useState(() => setupCheckId(project.id, true));
  const [busy, setBusy] = useState<'token' | 'check' | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [errorStep, setErrorStep] = useState<'token' | 'check'>('check');
  const [checked, setChecked] = useState(false);
  const [receipt, setReceipt] = useState<Awaited<ReturnType<typeof findSetupEvent>>>(null);
  const revision = useRef(0);
  const resetCheck = () => { revision.current += 1; setCheckId(setupCheckId(project.id, true)); setChecked(false); setReceipt(null); setError(null); };
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
  const snippets = setupSnippets(platform, token || 'st_YOUR_INGEST_TOKEN', origin, release, checkId);
  const native = platform === 'ios' || platform === 'android';
  const readable = receipt ? hasReadableSource(receipt.event) : false;
  const local = /^(https?:\/\/)?(localhost|127\.|\[::1\])/.test(origin);
  const createToken = () => {
    if (busy) return;
    setBusy('token'); setErrorStep('token'); setError(null);
    void api.createToken(project.id, `${platform} setup`, 'ingest').then(
      (r) => { if (alive.current) { setToken(r.token); resetCheck(); } },
      (err: unknown) => { if (alive.current) setError(formatApiError(err)); },
    ).finally(() => { if (alive.current) setBusy(null); });
  };
  const verify = () => {
    if (busy) return;
    const current = revision.current;
    setBusy('check'); setErrorStep('check'); setError(null);
    void findSetupEvent(project.id, checkId).then(
      (r) => { if (alive.current && revision.current === current) { setReceipt(r); setChecked(true); } },
      (err: unknown) => { if (alive.current && revision.current === current) setError(formatApiError(err)); },
    ).finally(() => { if (alive.current) setBusy(null); });
  };
  const [step, setStep] = useState('install');
  const [installed, setInstalled] = useState(false);
  const steps = ['install','configure','verify'] as const;
  const configurationReady = (() => {try {const url = new URL(origin); return ['http:','https:'].includes(url.protocol) && !url.username && !url.password && !!release.trim() && token.trim().startsWith('st_') && token.trim().length >= 29;} catch {return false;}})();
  const statusLabels = {pending:t('setup.stepPending'),current:t('setup.stepCurrent'),complete:t('setup.stepComplete'),busy:t('setup.checking'),error:t('setup.stepError'),blocked:t('setup.stepBlocked'),skipped:t('setup.stepSkipped')};
  const stepTitle = {install:t('setup.installTitle'),configure:t('setup.configureTitle'),verify:t('setup.verifyTitle')};
  const stepDescription = {install:t('setup.installStepHint'),configure:t('setup.configureStepHint'),verify:t('setup.testHint')};
  return <>
    <div className="control-bar flow-configuration">
      <Field label={t('setup.platform')}><Select value={platform} disabled={Boolean(busy)} onChange={event => {setPlatform(setupPlatform(event.target.value)); setInstalled(false); setStep('install'); resetCheck();}}>{SETUP_PLATFORMS.map(value => <option key={value} value={value}>{platformLabel(value,t)}</option>)}</Select></Field>
      <Field label={t('setup.release')}><Input value={release} disabled={Boolean(busy)} onChange={event => {setRelease(event.target.value); resetCheck();}} /></Field>
    </div>
    <FlowSteps label={t('nav.setup')} current={step} statusLabels={statusLabels} optionalLabel={t('setup.optional')} steps={steps.map(id => ({id,label:stepTitle[id],description:stepDescription[id],status:id === 'install' && installed ? 'complete' : id === 'configure' && configurationReady ? 'complete' : id === 'verify' && receipt ? 'complete' : id === step ? busy ? 'busy' : error ? 'error' : 'current' : 'pending',panelId:'sdk-setup-panel'}))} canGo={target => !busy && (target !== 'verify' || configurationReady)} onGo={setStep} />
    <FlowFrame title={stepTitle[step as keyof typeof stepTitle]} description={stepDescription[step as keyof typeof stepDescription]} panelId="sdk-setup-panel" stepKey={step} state={busy ?? (receipt ? 'received' : 'pending')} asideTitle={t('setup.progressTitle')} asideDescription={t('setup.progressHint')} aside={<div className="form-stack">
      <ul className="form-stack"><Progress done={Boolean(receipt)} label={t('setup.receiptStatus')} /><Progress done={readable} label={t('setup.sourceStatus')} /></ul>
      {receipt && !readable && <p className="muted text-sm">{t('setup.sourceHint')}</p>}
      {receipt && <Link to={`/issues/${receipt.issueId}?project=${project.id}`} className="text-link">{t('setup.openIssue')}<ArrowRight size={16} aria-hidden /></Link>}
      <Surface title={t('setup.nextTitle')}><p className="muted text-sm">{t('setup.nextHint')}</p><div className="form-stack mt-4"><Link to="/docs/recipes/sourcemap-upload" className="text-link">{t('setup.symbolGuide')}</Link><Link to={`/settings/notifications?project=${project.id}`} className="text-link">{t('setup.alerts')}</Link></div></Surface>
    </div>} actions={<FlowActions back={{label:t('setup.back'),disabled:step === 'install' || Boolean(busy),onClick:() => setStep(step === 'verify' ? 'configure' : 'install')}} next={{label:t(step === 'verify' ? 'setup.check' : 'setup.next'),busy:busy === 'check',disabled:Boolean(busy) || step !== 'install' && !configurationReady,onClick:() => {if (step === 'verify') verify(); else {if (step === 'install') setInstalled(true); setStep(step === 'install' ? 'configure' : 'verify');}}}} summary={<span>{steps.indexOf(step as typeof steps[number]) + 1} / 3</span>} />}>
      {step === 'install' && <div className="form-stack">
        <CopyCode code={snippets.install} label={t(native ? 'setup.package' : 'setup.terminal')} />
        <p className="muted text-sm">{t(`setup.install.${platform === 'react-native' ? 'rn' : platform}`)}</p>
        <Link to={SETUP_DOCS[platform]} className="text-link">{t('setup.platformGuide')}<ExternalLink size={15} aria-hidden /></Link>
      </div>}
      {step === 'configure' && <div className="form-stack">
        <Field label={t('setup.endpoint')}><Input type="url" value={origin} disabled={Boolean(busy)} onChange={event => {setOrigin(event.target.value); resetCheck();}} /></Field>
        <Field label={t('setup.ingestKey')} hint={t('setup.keyHint')}><Input type="password" autoComplete="off" spellCheck={false} value={token} disabled={Boolean(busy)} onChange={event => {setToken(event.target.value); resetCheck();}} placeholder="st_…" /></Field>
        <div className="flow-inline-actions"><DesignButton onClick={createToken} busy={busy === 'token'} disabled={Boolean(busy) || Boolean(token)}>{t('setup.createKey')}</DesignButton></div>
        {local && <p className="muted text-sm">{t('setup.localEndpoint')}</p>}
        {error && errorStep === 'token' && <ErrorBanner reason={error}>{t('setup.failed')}</ErrorBanner>}
        <CopyCode code={snippets.configure} label={t('setup.appStartup')} />
        <p className="muted text-sm">{t('setup.developmentHint')}</p>
      </div>}
      {step === 'verify' && <div className="form-stack">
        <CopyCode code={snippets.test} label={t('setup.testAction')} />
        <p role="status" className="muted text-sm">{receipt ? t('setup.received') : checked ? t('setup.waiting') : t('setup.notChecked')}</p>
        {error && errorStep === 'check' && <ErrorBanner reason={error}>{t('setup.failed')}</ErrorBanner>}
        {checked && !receipt && <p className="muted text-sm">{t('setup.waitingHint')}</p>}
      </div>}
    </FlowFrame>
  </>;
}

function Progress({ done, label }: { done: boolean; label: string }) {
  return <li className="flex items-center gap-3">{done ? <Check className="h-4 w-4 text-ok" aria-hidden /> : <Circle className="h-4 w-4 text-fg-subtle" aria-hidden />}<span className={done ? 'text-fg' : 'text-fg-muted'}>{label}</span></li>;
}
