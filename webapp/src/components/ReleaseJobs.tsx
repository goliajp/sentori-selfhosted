import { useEffect, useRef, useState } from 'react';
import { Surface } from '../../shared/design/Surface';
import { Badge, Button } from '../../shared/design/controls';
import { useLocale, useT } from '../i18n';
import { api, type JobRow } from '../lib/api';
import { formatApiError } from '../lib/useAsyncData';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';
import { ErrorBanner, TimeAgo } from './ui';

export function ReleaseJobs({ projectId, releaseId, revision }: { projectId: string; releaseId: string; revision: number }) {
  const t = useT(), locale = useLocale();
  const jobs = useScopedAsyncData(JSON.stringify([projectId, releaseId, revision]), () => api.listJobs(projectId, releaseId));
  const [busy, setBusy] = useState(false), [failure, setFailure] = useState<string | null>(null);
  const lock = useRef(false), requestKey = useRef<string | null>(null);
  const active = jobs.data?.jobs.some(job => ['queued', 'running', 'retry_wait'].includes(job.state));
  const { reload } = jobs;
  useEffect(() => {
    if (!active || jobs.loading) return;
    const timer = window.setTimeout(reload, 2000);
    return () => window.clearTimeout(timer);
  }, [active, jobs.data, jobs.loading, reload]);
  async function act(job?: JobRow) {
    if (lock.current) return;
    lock.current = true; setBusy(true); setFailure(null);
    try {
      if (job) await api.cancelJob(projectId, job.id);
      else {
        // Reuse the key after an ambiguous failure to recover the same command.
        requestKey.current ??= Array.from(crypto.getRandomValues(new Uint8Array(16)), value => value.toString(16).padStart(2, '0')).join('');
        await api.resymbolicateRelease(projectId, releaseId, requestKey.current);
        requestKey.current = null;
      }
      reload();
    } catch (error) { setFailure(formatApiError(error)); }
    finally { lock.current = false; setBusy(false); }
  }
  const states = {
    queued: t('jobs.queued'), running: t('jobs.running'), retry_wait: t('jobs.retryWait'),
    succeeded: t('jobs.succeeded'), failed: t('jobs.failed'), cancelled: t('jobs.cancelled'),
  };
  const reasons: Record<string, string> = {
    processing_failed: t('jobs.processingFailed'), batch_timeout: t('jobs.timeout'),
    attempts_exhausted: t('jobs.exhausted'), generation_changed: t('jobs.generationChanged'),
  };
  return <Surface title={t('jobs.title')}>
    <div className="form-stack">
      <p className="muted text-sm">{t('jobs.hint')}</p>
      <div className="flex flex-wrap gap-2">
        <Button small busy={busy} onClick={() => void act()}>{t('jobs.run')}</Button>
        <Button small disabled={jobs.loading} onClick={reload}>{t('jobs.refresh')}</Button>
      </div>
      {(failure || jobs.error) && <ErrorBanner reason={failure || jobs.error || ''}>{t('jobs.actionFailed')}</ErrorBanner>}
      {jobs.data?.jobs.length === 0 && <p className="muted text-sm">{t('jobs.empty')}</p>}
      {jobs.data?.jobs.map(job => <div key={job.id} className="section-stack" data-job-id={job.id}>
        <div className="flex flex-wrap items-center justify-between gap-2">
          <Badge tone={job.state === 'succeeded' ? 'success' : job.state === 'failed' ? 'danger' : 'neutral'}>{states[job.state]}</Badge>
          <TimeAgo iso={job.createdAt}/>
        </div>
        <p className="text-sm" aria-live={active ? 'polite' : 'off'}>{t('jobs.progress', { scanned: job.scanned.toLocaleString(locale), rewritten: job.rewritten.toLocaleString(locale), frames: job.frames.toLocaleString(locale) })}</p>
        {job.attempt > 0 && <p className="subtle text-xs">{t('jobs.attempt', { attempt: String(job.attempt), max: String(job.maxAttempts) })}</p>}
        {job.scanned === job.scanLimit && <p className="muted text-sm">{t('jobs.limit')}</p>}
        {job.errorCode && <p className="text-sm text-danger">{reasons[job.errorCode] ?? t('jobs.processingFailed')}</p>}
        {['queued', 'running', 'retry_wait'].includes(job.state) && <div><Button small disabled={busy || job.cancelRequested} onClick={() => void act(job)}>{t(job.cancelRequested ? 'jobs.cancelling' : 'jobs.cancel')}</Button></div>}
      </div>)}
    </div>
  </Surface>;
}
