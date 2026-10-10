import { useIntegrationCommand } from '../lib/useIntegrationCommand';
import { useState } from 'react';
import { Link } from 'react-router-dom';
import { Surface } from '../../shared/design/Surface';
import { Button } from '../../shared/design/controls';
import { Dialog } from '../../shared/design/Dialog';
import { useT } from '../i18n';
import { useShell } from '../App';
import { api } from '../lib/api';
import { commandId } from '../lib/team-contract';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';
import type { Integration } from '../lib/integration-contract';
import { ErrorBanner } from './ui';

export function SendIntegration({ project, integration, task, onClose, saved }: { project: string; integration: Integration; task?: { id: string; title: string; description: string; version: number }; onClose: () => void; saved: () => void }) {
  const t = useT(), command = useIntegrationCommand(project, saved), locked = command.busy || command.uncertain;
  const native = ['github', 'jira'].includes(integration.kind);
  return <Dialog open title={t(task ? 'integration.exportTask' : 'integration.sendTest')} closeLabel={t('common.close')} onClose={() => { if (!locked) onClose(); }} footer={<><Button disabled={locked} onClick={onClose}>{t('team.cancel')}</Button><Button variant="primary" busy={command.busy} onClick={() => void command.run({ type: 'send', id: integration.id, body: { commandId: commandId(), expectedVersion: integration.version, taskId: task?.id ?? null, expectedTaskVersion: task?.version ?? null } })}>{command.uncertain ? t('common.retry') : t('integration.confirmSend')}</Button></>}>
    <div className="form-stack"><p>{t(native ? 'integration.createsTicket' : 'integration.realSend')}</p><dl className="team-definition"><dt>{t('integration.destination')}</dt><dd>{integration.summary.repository || integration.summary.endpoint || integration.summary.endpointOrigin || integration.summary.recipient}</dd>{task && <><dt>{t('team.taskTitle')}</dt><dd>{task.title}</dd></>}</dl>{task && <p className="max-h-48 overflow-auto whitespace-pre-wrap text-sm">{task.description}</p>}<p className="muted text-sm">{t('integration.queueHint')}</p>{command.error && <ErrorBanner reason={command.error}>{t('team.failed')}</ErrorBanner>}{command.uncertain && <p role="status">{t('team.uncertain')}</p>}</div>
  </Dialog>;
}
export function ExternalTask({ project, task }: { project: string; task: { id: string; title: string; description: string; version: number } }) {
  const t = useT(), { me } = useShell(), list = useScopedAsyncData(project, () => api.listIntegrations(project));
  const [chosen, setChosen] = useState<Integration | null>(null), [sent, setSent] = useState(false);
  return <Surface title={t('integration.exportTask')}><div className="form-stack"><p className="muted text-sm">{t('integration.exportHint')}</p>{list.error && <ErrorBanner reason={list.error}>{t('team.failed')}</ErrorBanner>}{list.data?.integrations.filter(i => ['github', 'jira'].includes(i.kind) && i.enabled).map(i => <Button key={i.id} disabled={me.role !== 'superadmin' || !list.data?.vaultConfigured} onClick={() => { setSent(false); setChosen(i); }}>{i.kind === 'github' ? 'GitHub' : 'Jira'}</Button>)}{sent && <p role="status">{t('integration.queuedHint')}</p>}<Link className="text-link" to={`/integrations?project=${project}`}>{t('integration.title')}</Link></div>{chosen && <SendIntegration project={project} integration={chosen} task={task} onClose={() => setChosen(null)} saved={() => { setChosen(null); setSent(true); }}/>}</Surface>;
}
