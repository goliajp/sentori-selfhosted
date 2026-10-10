import { Link } from 'react-router-dom';
import { Surface } from '../../shared/design/Surface';
import { Button } from '../../shared/design/controls';
import { AuthenticationMode } from '../components/AuthenticationMode';
import { ErrorBanner, formatAbsolute } from '../components/ui';
import { useLocale, useT } from '../i18n';
import { api } from '../lib/api';
import { useAsyncData } from '../lib/useAsyncData';

export function CloudWorkspace({ owner }: { owner: boolean }) {
  const t = useT(), locale = useLocale();
  const usage = useAsyncData(() => owner ? api.cloudUsage() : Promise.resolve(null), [owner]);
  const value = usage.data;
  const number = new Intl.NumberFormat(locale);
  const rows = value ? [
    [t('cloud.events'), value.events, value.limits.monthlyEvents],
    [t('cloud.replays'), value.replays, value.limits.monthlyReplays],
    [t('cloud.ingestBytes'), value.ingestBytes, value.limits.monthlyIngestBytes],
    [t('cloud.artifactBytes'), value.artifactBytes, value.limits.artifactBytes],
    [t('nav.projects'), value.projects, value.limits.projects],
  ] as const : [];
  return <div className="form-stack">
    <Surface title={t('transfer.title')}><p className="host-copy">{t('transfer.description')}</p><Link className="text-link" to="/transfers">{t('transfer.title')}</Link></Surface>
    <Surface title={t('cloud.workspace')}><AuthenticationMode account><p>{t('auth.cloudManaged')}</p></AuthenticationMode></Surface>
    {owner && <Surface title={t('cloud.usage')} action={<Button disabled={usage.loading} onClick={usage.reload}>{t('instance.refresh')}</Button>}>
      {usage.error && <ErrorBanner reason={usage.error}>{t('cloud.usageFailed')}</ErrorBanner>}
      {usage.loading && !value && <p role="status">{t('shell.loading')}</p>}
      {value && <>
        <p className="mb-4 text-sm text-fg-muted">{t('cloud.resetAt', {time: formatAbsolute(value.resetsAt)})}</p>
        {!value.active && <p role="status">{t('cloud.unavailable')}</p>}
        <dl className="grid grid-cols-[minmax(0,1fr)_auto] gap-x-4 gap-y-3 text-sm">{rows.map(([label, used, limit]) => <div className="contents" key={label}><dt>{label}</dt><dd className="tabular-nums">{number.format(used)} / {number.format(limit)}</dd></div>)}</dl>
        <p className="mt-4 text-sm text-fg-muted">{t('cloud.retention', {events:String(value.limits.eventRetentionDays), replays:String(value.limits.replayRetentionDays), rate:String(value.limits.eventsPerSecond)})}</p>
        <p className="mt-2 text-sm text-fg-muted">{t('cloud.trafficHint')}</p>
      </>}
    </Surface>}
  </div>;
}
