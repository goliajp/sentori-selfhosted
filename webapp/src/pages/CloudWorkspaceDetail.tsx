import { useEffect, useRef, useState } from 'react';
import { useParams, useSearchParams } from 'react-router-dom';
import { Surface } from '../../shared/design/Surface';
import { Button } from '../../shared/design/controls';
import { Dialog } from '../../shared/design/Dialog';
import { ErrorBanner, Field, Input, Select, formatAbsolute } from '../components/ui';
import { useLocale, useT } from '../i18n';
import { cloudApi, type CloudAccount, type CloudQuote } from '../lib/cloud-api';
import { formatApiError, useAsyncData } from '../lib/useAsyncData';

export function CloudWorkspaceDetail({account}: {account:CloudAccount}) {
  const t = useT(), locale = useLocale(), {workspaceId = ''} = useParams(), [params] = useSearchParams(), org = params.get('organization') ?? '';
  const data = useAsyncData(async () => {
    const [info, organization, status, catalog] = await Promise.all([cloudApi.workspace(org,workspaceId), cloudApi.organization(org),cloudApi.status(),cloudApi.catalog()]);
    const owner = organization.members.some(row => row.id === account.id && row.role === 'owner');
    const [quotes,billing] = owner ? await Promise.all([cloudApi.quotes(org,workspaceId),cloudApi.billing(org,workspaceId)]) : [null,null];
    return {info, organization, owner, status, catalog:catalog.catalog, quotes, billing};
  }, [org,workspaceId,account.id]);
  const [busy,setBusy] = useState(false), [failure,setFailure] = useState<string|null>(null), locked = useRef(false);
  const [name,setName] = useState('');
  const [offer,setOffer] = useState(''), [member,setMember] = useState(''), [role,setRole] = useState<'admin'|'member'>('member');
  const [renewal,setRenewal] = useState<'cancelRenewal'|'resumeRenewal'|null>(null);
  const value = data.data, workspace = value?.info.workspace, base = '/organizations/'+encodeURIComponent(org)+'/workspaces/'+encodeURIComponent(workspaceId);
  const active = !!value && (['checkout','paid','provisioning'].includes(value.info.workspace.status) || value.billing?.commands.some(row => ['queued','running'].includes(row.status)));
  useEffect(() => { if (!active) return; const timer = window.setInterval(data.reload,4000); return () => window.clearInterval(timer); },[active,data.reload]);
  async function run(suffix:string,body:Record<string,unknown>) {
    if (locked.current) return;
    locked.current = true; setBusy(true); setFailure(null);
    try { await cloudApi.command(account.id,base+suffix,body); setRenewal(null); setName(''); data.reload(); }
    catch (error) { setFailure(formatApiError(error)); }
    finally { locked.current = false; setBusy(false); }
  }
  async function enter() {
    if (locked.current) return;
    locked.current = true; setBusy(true); setFailure(null);
    try { location.assign(await cloudApi.entry(workspaceId)); }
    catch (error) { setFailure(formatApiError(error)); locked.current = false; setBusy(false); }
  }
  function money(amount:number,currency:string) {
    const formatter = new Intl.NumberFormat(locale,{style:'currency',currency});
    return formatter.format(amount / 10 ** (formatter.resolvedOptions().maximumFractionDigits ?? 2));
  }
  const latest = value?.billing?.checkouts.find(row => row.status !== 'expired');
  const quote = value?.quotes?.items.find(row => !row.expired);
  const pendingCommand = value?.billing?.commands.some(row => row.status !== 'succeeded');
  const selectedOffer = offer || value?.catalog?.offers[0]?.id || '';
  return <div className="form-stack">
    <div><Button disabled={data.loading || busy} onClick={data.reload}>{t('instance.refresh')}</Button></div>
    {(data.error || failure) && <ErrorBanner reason={data.error || failure}>{t('cloud.actionFailed')}</ErrorBanner>}
    {data.loading && !value && <p role="status">{t('shell.loading')}</p>}
    {value && workspace && <>
      <Surface title={workspace.name}><dl className="host-facts"><div><dt>{t('cloud.region')}</dt><dd>{workspace.region}</dd></div><div><dt>{t('instance.status')}</dt><dd>{t(`cloud.state.${workspace.status}`)}</dd></div></dl>
        {value.info.serviceGrant && <p className="mb-3">{t(value.info.serviceGrant.kind === 'internal' ? 'cloud.internalWorkspace' : 'cloud.trialWorkspace')}{value.info.serviceGrant.expiresAt && <> · <time dateTime={value.info.serviceGrant.expiresAt}>{formatAbsolute(value.info.serviceGrant.expiresAt)}</time></>}</p>}
        {workspace.status === 'ready' && <Button disabled={busy} onClick={() => void enter()}>{t('cloud.enter')}</Button>}
        {['paid','provisioning'].includes(workspace.status) && <p>{t(value.info.provisioning?.state === 'failed' ? 'cloud.provisioningFailed' : 'cloud.provisioningWait')}</p>}
        {value.owner && value.info.provisioning?.state === 'failed' && <Button disabled={busy} onClick={() => void run('/commands',{expectedVersion:workspace.version,action:'retryProvisioning'})}>{t('common.retry')}</Button>}
      </Surface>
      {value.owner && workspace.status === 'draft' && value.catalog && <Surface title={t('cloud.quote')}>
        <form className="form-stack max-w-xl" onSubmit={event => {event.preventDefault(); void run('/quotes',{expectedVersion:workspace.version,offerId:selectedOffer,catalogRevision:value.catalog!.revision});}}>
          <Field label={t('cloud.plan')}><Select value={selectedOffer} disabled={busy} onChange={event => setOffer(event.target.value)}>{value.catalog.offers.map(row => <option key={row.id} value={row.id}>{row.name} · {money(row.unitAmount,row.currency)} / {t(row.interval === 'month' ? 'cloud.month' : 'cloud.year')}</option>)}</Select></Field><Button type="submit" busy={busy}>{t('cloud.requestQuote')}</Button>
        </form>
        {quote && <QuoteCard key={quote.id} quote={quote} checkout={value.status.checkout} busy={busy} mode={value.status.billingMode} money={money} accept={() => void run('/checkout',{expectedVersion:workspace.version,quoteId:quote.id,termsRevision:quote.snapshot.terms.revision})}/>}
      </Surface>}
      {value.owner && value.billing && <Surface title={t('cloud.billing')}>
        {!value.status.checkout && <p>{t('cloud.checkoutUnavailable')}</p>}
        {value.status.billingMode && value.status.billingMode !== 'live' && <p className="mb-4 text-sm text-fg-muted">{t('cloud.testBilling')}</p>}
        {value.billing.checkouts.length === 0 && <p>{t('table.empty')}</p>}
        <ul className="form-stack">{value.billing.checkouts.map(row => <li key={row.id} className="border border-border rounded-lg p-4 form-stack">
          <p>{t('cloud.paymentStatus')}: <code>{row.status}</code>{row.subscriptionStatus && <> · <code>{row.subscriptionStatus}</code></>}</p>
          {row.paidUntil && <p>{t('cloud.paidUntil',{time:formatAbsolute(row.paidUntil)})}</p>}
          {row.error && <ErrorBanner reason={row.error}>{t('cloud.actionFailed')}</ErrorBanner>}
          {row.checkoutUrl && /^https:\/\/checkout\.stripe\.com\//.test(row.checkoutUrl) && <a className="button button-primary" href={row.checkoutUrl} referrerPolicy="no-referrer">{t('cloud.continueCheckout')}</a>}
          {row.status === 'attention' && <Button disabled={busy} onClick={() => void run('/billing/commands',{expectedVersion:workspace.version,action:'retryVerification'})}>{t('cloud.retryVerification')}</Button>}
        </li>)}</ul>
        {latest?.subscriptionStatus && !['canceled','incomplete_expired'].includes(latest.subscriptionStatus) && <div className="mt-4"><p className="mb-3">{t(latest.cancelAtPeriodEnd ? 'cloud.renewalCancelled':'cloud.renewalActive')}</p><Button disabled={busy || pendingCommand} onClick={() => setRenewal(latest.cancelAtPeriodEnd ? 'resumeRenewal':'cancelRenewal')}>{t(latest.cancelAtPeriodEnd ? 'cloud.resumeRenewal':'cloud.cancelRenewal')}</Button></div>}
        <ul className="form-stack mt-4">{value.billing.commands.map(row => <li key={row.id}><p>{t(row.cancelAtPeriodEnd ? 'cloud.cancelRenewal':'cloud.resumeRenewal')} · <code>{row.status}</code></p>{row.error && <p role="status">{row.error}</p>}{row.status === 'attention' && <Button disabled={busy} onClick={() => void run('/billing/commands',{expectedVersion:workspace.version,action:'retryCommand',id:row.id})}>{t('common.retry')}</Button>}</li>)}</ul>
        <h3 className="font-semibold mt-6 mb-3">{t('cloud.invoices')}</h3><ul className="form-stack">{value.billing.invoices.map(row => <li key={row.id} className="flex flex-wrap gap-3"><code className="break-all">{row.id}</code><span>{money(row.amountPaid,row.currency)}</span><span>{t('cloud.paidUntil',{time:formatAbsolute(row.paidUntil)})}</span></li>)}</ul>
      </Surface>}
      {value.owner && <Surface title={t('cloud.renameWorkspace')}><form className="form-stack max-w-xl" onSubmit={event => {event.preventDefault(); void run('/commands',{expectedVersion:workspace.version,action:'rename',name:name.trim()});}}><Field label={t('cloud.renameWorkspace')}><Input required maxLength={80} value={name} placeholder={workspace.name} disabled={busy} onChange={event => setName(event.target.value)}/></Field><Button type="submit" busy={busy} disabled={!name.trim()}>{t('cloud.renameWorkspace')}</Button></form></Surface>}
      <Surface title={t('cloud.workspaceMembers')}>
        <p className="text-sm text-fg-muted mb-4">{t('cloud.workspaceMembersHint')}</p>
        <ul className="form-stack">{value.info.members.map(row => <li key={row.id} className="flex flex-wrap items-center gap-3"><span className="min-w-0 flex-1 break-all">{row.email}</span><span>{t(row.role === 'admin' ? 'shell.roleOwner':'shell.roleAdmin')}</span>{value.owner && <Button disabled={busy} onClick={() => void run('/commands',{expectedVersion:workspace.version,action:'revoke',id:row.id})}>{t('cloud.revokeAccess')}</Button>}</li>)}</ul>
        {value.owner && <form className="form-stack mt-6 max-w-xl" onSubmit={event => {event.preventDefault(); void run('/commands',{expectedVersion:workspace.version,action:'grant',id:member,role});}}><Field label={t('cloud.members')}><Select required value={member} disabled={busy} onChange={event => setMember(event.target.value)}><option value="" disabled>{t('cloud.selectMember')}</option>{value.organization.members.filter(row => row.role !== 'owner').map(row => <option key={row.id} value={row.id}>{row.email}</option>)}</Select></Field><Field label={t('settings.colRole')}><Select value={role} disabled={busy} onChange={event => setRole(event.target.value as 'admin'|'member')}><option value="member">{t('shell.roleAdmin')}</option><option value="admin">{t('shell.roleOwner')}</option></Select></Field><Button type="submit" busy={busy} disabled={!member}>{t('cloud.grantAccess')}</Button></form>}
      </Surface>
      <Dialog open={renewal!==null} title={t(renewal === 'resumeRenewal' ? 'cloud.resumeRenewal':'cloud.cancelRenewal')} closeLabel={t('common.close')} onClose={() => {if (!busy) setRenewal(null);}} footer={<><Button disabled={busy} onClick={() => setRenewal(null)}>{t('common.close')}</Button><Button variant="primary" busy={busy} onClick={() => renewal && void run('/billing/commands',{expectedVersion:workspace.version,action:renewal})}>{t(renewal === 'resumeRenewal' ? 'cloud.resumeRenewal':'cloud.cancelRenewal')}</Button></>}><p>{t('cloud.renewalHint')}</p>{failure && <ErrorBanner reason={failure}>{t('cloud.actionFailed')}</ErrorBanner>}</Dialog>
    </>}
  </div>;
}

function QuoteCard({quote,checkout,busy,mode,money,accept}:{quote:CloudQuote;checkout:boolean;busy:boolean;mode:string|null;money:(amount:number,currency:string)=>string;accept:()=>void}) {
  const t = useT(), [agreed,setAgreed] = useState(false), offer = quote.snapshot.offer, terms = quote.snapshot.terms;
  const [expired,setExpired] = useState(() => Date.parse(quote.acceptUntil) <= Date.now());
  useEffect(() => { const timer = window.setTimeout(() => setExpired(true),Math.max(0,Date.parse(quote.acceptUntil)-Date.now())); return () => window.clearTimeout(timer); },[quote.acceptUntil]);
  return <div className="form-stack mt-6 border-t border-border pt-6">
    <h3 className="font-semibold">{offer.name}</h3><p>{money(offer.unitAmount,offer.currency)} / {t(offer.interval === 'month' ? 'cloud.month':'cloud.year')} · {t(offer.taxBehavior === 'inclusive' ? 'cloud.taxIncluded':'cloud.taxExtra')}</p>
    <p>{t('cloud.quoteExpires',{time:formatAbsolute(quote.acceptUntil)})}</p>
    <dl className="host-facts">{[[t('cloud.events'),offer.limits.monthlyEvents],[t('cloud.replays'),offer.limits.monthlyReplays],[t('cloud.ingestBytes'),offer.limits.monthlyIngestBytes],[t('cloud.artifactBytes'),offer.limits.artifactBytes],[t('nav.projects'),offer.limits.projects]].map(([label,amount]) => <div key={label}><dt>{label}</dt><dd>{amount}</dd></div>)}</dl>
    <p>{t('cloud.retention',{events:String(offer.limits.eventRetentionDays),replays:String(offer.limits.replayRetentionDays),rate:String(offer.limits.eventsPerSecond)})}</p><p>{terms.cancellation}</p>
    <div className="flex flex-wrap gap-4">{[[t('cloud.serviceTerms'),terms.serviceUrl],[t('cloud.privacy'),terms.privacyUrl],[t('cloud.refunds'),terms.refundUrl]].map(([label,url]) => <a className="text-link" key={url} href={url} target="_blank" rel="noopener noreferrer">{label}</a>)}</div>
    <label className="choice-control"><input type="checkbox" checked={agreed} disabled={busy} onChange={event => setAgreed(event.target.checked)}/><span>{t('cloud.agreeTerms',{revision:terms.revision})}</span></label>
    {mode !== 'live' && <p>{t('cloud.testBilling')}</p>}
    <Button variant="primary" busy={busy} disabled={!agreed || !checkout || expired} onClick={accept}>{t('cloud.acceptQuote')}</Button>
  </div>;
}
