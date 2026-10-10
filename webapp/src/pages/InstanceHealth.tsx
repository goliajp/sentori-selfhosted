import { useRef, useState } from 'react';
import { Button, ErrorBanner, Panel, TimeAgo } from '../components/ui';
import { useT } from '../i18n';
import { api } from '../lib/api';
import type { InstanceHealth as Report } from '../lib/system-contract';
import { formatApiError } from '../lib/useAsyncData';

const checks = ['database', 'schema', 'instance', 'backup', 'operations', 'storage', 'recovery'] as const;
const reasons = ['database_reachable', 'database_unavailable', 'metadata_unavailable',
  'schema_matches_binary', 'schema_mismatch', 'instance_ready', 'recovery_validation_active',
  'setup_incomplete', 'instance_identity_invalid', 'recent_backup_record', 'backup_record_stale',
  'backup_clock_invalid', 'backup_record_missing', 'lifecycle_operation_stalled',
  'no_stalled_lifecycle_operations', 'storage_not_persistent', 'storage_directory_readable',
  'storage_directory_unreadable', 'recovery_not_verified'] as const;

export function InstanceHealthPanel() {
  const t = useT();
  const [report, setReport] = useState<Report | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const locked = useRef(false);
  async function run() {
    if (locked.current) return;
    locked.current = true; setBusy(true); setError(null); setReport(null);
    try { setReport(await api.instanceHealth()); }
    catch (failure) { setError(formatApiError(failure)); }
    finally { locked.current = false; setBusy(false); }
  }
  return <Panel title={t('instanceHealth.title')} padded>
    <div className="space-y-3" data-testid="instance-health">
      <p className="text-sm text-fg-muted">{t('instanceHealth.hint')}</p>
      <Button disabled={busy} onClick={() => void run()}>{t('instanceHealth.run')}</Button>
      {error && <ErrorBanner reason={error}>{t('instance.actionFailed')}</ErrorBanner>}
      {report && <>
        <p role="status">{t(`instanceHealth.status.${report.status}`)} · <TimeAgo iso={report.checkedAt} /></p>
        <dl className="space-y-3">
          {report.checks.map(check => {
            const id = checks.find(id => id === check.id);
            const reason = reasons.find(code => code === check.code);
            return <div key={check.id} className="space-y-1">
              <dt className="text-sm">{id ? t(`instanceHealth.check.${id}`) : t('instanceHealth.unknownCheck')} · {t(`instanceHealth.status.${check.status}`)}</dt>
              <dd className="text-sm text-fg-muted">{reason ? t(`instanceHealth.reason.${reason}`) : t('instanceHealth.unknownReason')}</dd>
            </div>;
          })}
        </dl>
      </>}
    </div>
  </Panel>;
}
