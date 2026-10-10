import { useEffect, useRef, useState } from 'react';
import { Link, useLocation, useNavigate } from 'react-router-dom';
import { Button } from '../../shared/design/controls';
import { AuthShell } from '../components/AuthShell';
import { ErrorBanner, Field, Input } from '../components/ui';
import { useT } from '../i18n';
import { cloudApi, cloudReturnTo } from '../lib/cloud-api';
import { formatApiError, useAsyncData } from '../lib/useAsyncData';

export default function CloudAuth() {
  const t = useT(), navigate = useNavigate(), path = useLocation().pathname;
  const mode = path.endsWith('/register') ? 'register' : path.endsWith('/forgot-password') ? 'reset' : path.endsWith('/verify') ? 'verify' : 'login';
  const status = useAsyncData(() => cloudApi.status(), []);
  const [email, setEmail] = useState(''), [password, setPassword] = useState(''), [confirm, setConfirm] = useState(''), [name, setName] = useState('');
  const [proof, setProof] = useState<{token:string; purpose:string}|null>(null), captured = useRef(false);
  const [busy, setBusy] = useState(false), [done, setDone] = useState(false), [failure, setFailure] = useState<string|null>(null);
  const locked = useRef(false);
  useEffect(() => {
    if (mode !== 'verify' || captured.current) return;
    captured.current = true;
    const values = new URLSearchParams(location.hash.slice(1));
    history.replaceState(null, '', location.pathname + location.search);
    const token = values.get('token'), purpose = values.get('purpose');
    if (token && /^[a-f0-9]{64}$/.test(token) && (purpose === 'register' || purpose === 'reset')) {
      Promise.resolve().then(() => setProof({token,purpose}));
    }
  }, [mode]);
  async function submit(event: React.FormEvent) {
    event.preventDefault(); if (locked.current) return;
    if (mode === 'verify' && password !== confirm) { setFailure(t('auth.passwordsDiffer')); return; }
    locked.current = true; setBusy(true); setFailure(null);
    try {
      if (mode === 'login') { await cloudApi.login(email, password); navigate(cloudReturnTo(), {replace:true}); }
      else if (mode === 'verify' && proof) { await cloudApi.complete(proof.token, proof.purpose, password, proof.purpose === 'reset' ? 'Account' : name); setProof(null); setPassword(''); setConfirm(''); setDone(true); }
      else if (mode === 'register' || mode === 'reset') { await cloudApi.requestLink(email, mode); setDone(true); }
    } catch (error) { setFailure(formatApiError(error)); }
    finally { locked.current = false; setBusy(false); }
  }
  const title = t(mode === 'register' ? 'cloud.register' : mode === 'reset' ? 'auth.forgot' : mode === 'verify' ? 'cloud.verify' : 'cloud.signIn');
  return <AuthShell><form className="auth-form" onSubmit={submit}>
    <h1 className="text-lg font-semibold">{title}</h1>
    {status.error && <><ErrorBanner reason={status.error}>{t('auth.modeFailed')}</ErrorBanner><Button onClick={status.reload}>{t('common.retry')}</Button></>}
    {done ? <p role="status">{t(mode === 'verify' ? 'auth.resetDone' : 'cloud.linkSent')}</p> : <>
      {(mode === 'register' || mode === 'reset') && <p>{t('cloud.emailFirst')}</p>}
      {mode !== 'verify' && <Field label={t('auth.email')}><Input type="email" required autoComplete="username" value={email} onChange={event => setEmail(event.target.value)} disabled={busy}/></Field>}
      {mode === 'verify' && proof?.purpose === 'register' && <Field label={t('cloud.displayName')}><Input required maxLength={80} autoComplete="name" value={name} onChange={event => setName(event.target.value)} disabled={busy}/></Field>}
      {(mode === 'login' || mode === 'verify') && <Field label={t('auth.password')}><Input type="password" required minLength={mode === 'verify' ? 12 : undefined} maxLength={1024} autoComplete={mode === 'verify' ? 'new-password' : 'current-password'} value={password} onChange={event => setPassword(event.target.value)} disabled={busy}/></Field>}
      {mode === 'verify' && <Field label={t('auth.confirm')}><Input type="password" required minLength={12} maxLength={1024} autoComplete="new-password" value={confirm} onChange={event => setConfirm(event.target.value)} disabled={busy}/></Field>}
      {mode === 'verify' && !proof && <p role="status">{t('auth.missingToken')}</p>}
      {(mode === 'register' || mode === 'reset') && status.data && !status.data.registration && <p role="status">{t('cloud.mailUnavailable')}</p>}
      {failure && <ErrorBanner reason={failure}>{t('cloud.actionFailed')}</ErrorBanner>}
      <Button type="submit" variant="primary" size="large" busy={busy} disabled={!status.data || (mode === 'verify' && !proof) || ((mode === 'register' || mode === 'reset') && !status.data.registration)}>{t(mode === 'login' ? 'auth.signIn' : mode === 'verify' ? 'auth.setNewPassword' : 'cloud.sendLink')}</Button>
    </>}
    <div className="flex flex-wrap gap-4 text-sm"><Link className="text-link" to="/cloud/login">{t('cloud.signIn')}</Link><Link className="text-link" to="/cloud/register">{t('cloud.register')}</Link><Link className="text-link" to="/cloud/forgot-password">{t('auth.forgot')}</Link></div>
  </form></AuthShell>;
}
