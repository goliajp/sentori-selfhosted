import { useCallback } from 'react';
import { Picker } from '../../shared/design/Picker';
import { useLocale, useT } from '../i18n';
import { api } from '../lib/api';
import { releaseDisplayName } from '../lib/release-name';
import { useReleaseNames } from '../lib/useReleaseNames';
import { platformLabel } from '../lib/platform-label';

export function ReleasePicker({projectId, appId, value, onChange, disabled, placeholder}: {
  projectId: string; appId?: string; value: string; onChange: (value: string) => void; disabled?: boolean; placeholder?: string;
}) {
  const t = useT(), locale = useLocale();
  const names = useReleaseNames(projectId, [value]);
  const loadOptions = useCallback(async (query: string, {signal}: {signal: AbortSignal}) => {
    const result = await api.listReleases(projectId, {appId, q: query, limit: 100}, signal);
    return result.releases.map(row => ({value: row.name, label: releaseDisplayName(row, t, {appId}),
      description: [...new Set((row.apps ?? []).filter(app => !appId || app.id === appId).map(app => platformLabel(app.platform, t)))].join(' · '), keywords: [row.name]}));
  }, [projectId, appId, t]);
  return <Picker key={projectId+':'+(appId??'')} multiple={false} searchable clearable locale={locale}
    aria-label={t('inbox.releaseFilter')} value={value} onChange={onChange} disabled={disabled}
    placeholder={placeholder ?? t('queue.all')} loadOptions={loadOptions}
    selectedOptions={value ? [{value, label: names.label(value, {appId})}] : []}/>;
}
