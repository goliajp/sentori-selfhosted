import { useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { Button } from '../../shared/design/controls';
import { AuthShell } from '../components/AuthShell';
import { ErrorBanner } from '../components/ui';
import { useT } from '../i18n';
import { cloudApi } from '../lib/cloud-api';
import { formatApiError, useAsyncData } from '../lib/useAsyncData';
export default function CloudAccess() {
  const t = useT(), [params] = useSearchParams(), workspace = params.get('workspaceId') ?? '', challenge = params.get('challenge') ?? '';
  const valid = /^[a-f0-9-]{36}$/.test(workspace) && /^[a-f0-9]{64}$/.test(challenge);
  const info = useAsyncData(async () => {
    if (!valid) throw new Error(t('cloud.accessInvalid'));
    const [account, access] = await Promise.all([cloudApi.me(), cloudApi.access(workspace)]);
    return {account, access};
  }, [workspace, challenge, valid]);
  const [failure, setFailure] = useState<string|null>(null), [busy, setBusy] = useState(false), locked = useRef(false);
  async function enter() {
    if (!info.data || locked.current) return;
    locked.current = true; setBusy(true); setFailure(null);
    try {
      const result = await cloudApi.ticket(info.data.access.organizationId, workspace, challenge);
      const url = new URL(result.url), origin = new URL(info.data.access.origin);
      if (url.origin !== origin.origin || url.pathname !== '/auth/cloud/complete' || !url.hash.startsWith('#ticket=')) throw new Error(t('cloud.accessInvalid'));
      location.replace(url.href);
    } catch (error) { setFailure(formatApiError(error)); locked.current = false; setBusy(false); }
  }
  return <AuthShell><div className="auth-form"><h1 className="text-lg font-semibold">{t('cloud.enter')}</h1>
    {info.loading && <p role="status">{t('shell.loading')}</p>}
    {(info.error || failure) && <><ErrorBanner reason={info.error || failure}>{t('cloud.actionFailed')}</ErrorBanner><Button disabled={!valid || busy} onClick={info.reload}>{t('common.retry')}</Button></>}
    {info.data && <><p className="break-all">{info.data.account.email}</p><p className="font-semibold break-words">{info.data.access.name}</p><p className="text-sm text-fg-muted break-all">{info.data.access.origin}</p><Button variant="primary" size="large" busy={busy} onClick={() => void enter()}>{t('cloud.enter')}</Button></>}
    <Link className="text-link" to="/cloud">{t('cloud.console')}</Link>
  </div></AuthShell>;
}
