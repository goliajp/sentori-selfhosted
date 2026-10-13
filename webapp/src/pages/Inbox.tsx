import { Checkbox } from "../../shared/design/Checkbox";
import { useDocumentSearchParams } from '../lib/search-params';
import { WorkbenchEmpty } from '../components/WorkbenchEmpty';
import { useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { Archive, Check, Mail, Settings2 } from 'lucide-react';
import { Heading, Surface } from '../../shared/design/Surface';
import { Badge, Button } from '../../shared/design/controls';
import { Dialog } from '../../shared/design/Dialog';
import { SegmentedControl } from '../../shared/design/SegmentedControl';
import { useT } from '../i18n';
import { ErrorBanner, TimeAgo } from '../components/ui';
import { api } from '../lib/api';
import { formatApiError } from '../lib/useAsyncData';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';
import type { InboxPreferences } from '../lib/team-contract';

export default function InboxPage() {
  const currentSearchParams=useDocumentSearchParams();
  const t = useT(), [params, setParams] = useSearchParams();
  const requested = params.get('filter') ?? 'unread', filter = ['unread','all','archived'].includes(requested) ? requested : 'unread';
  const rawOffset = Number(params.get('offset') ?? 0), offset = Number.isSafeInteger(rawOffset) && rawOffset >= 0 ? rawOffset : 0;
  const query = new URLSearchParams({ offset: String(offset), archived: String(filter === 'archived'), unread: String(filter === 'unread') });
  const list = useScopedAsyncData(query.toString(), () => api.listInbox(query));
  const [error, setError] = useState<string | null>(null), [busy, setBusy] = useState(false), [prefsOpen, setPrefsOpen] = useState(false);
  const lock = useRef(false);
  const set = (key: string, value: string) => { const next = currentSearchParams(); next.set(key, value); if (key !== 'offset') next.delete('offset'); setParams(next); };
  async function update(id: string, change: { read?: boolean; archived?: boolean }) {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError(null);
    try { if (id) await api.updateInbox(id, change); else await api.readAllInbox(); list.reload(); } catch (failure) { setError(formatApiError(failure)); }
    finally { lock.current = false; setBusy(false); }
  }
  const stages: Record<string, string> = { triage: t('team.triage'), ready: t('team.ready'), inProgress: t('team.inProgress'), review: t('team.review'), verify: t('team.verify'), done: t('team.done'), cancelled: t('team.cancelled') };
  const categories = { assignment: t('team.assignment'), mention: t('team.mention'), task: t('team.taskUpdates'), monitor: t('team.monitors') };
  return <div className="production-page team-page">
    <Heading title={t('team.inbox')} description={t('team.inboxDescription')} action={<Button onClick={() => setPrefsOpen(true)}><Settings2 size={15}/>{t('team.preferences')}</Button>}/>
    <div className="team-toolbar"><SegmentedControl label={t('team.inbox')} value={filter} onChange={v => set('filter', v)} options={[["unread", t('team.unread')], ["all", t('team.all')], ["archived", t('team.archived')]]}/><span className="team-toolbar-spacer"/><Button disabled={busy || !list.data?.unread} onClick={() => void update('', {})}>{t('team.markAllRead')}</Button><Button onClick={list.reload}>{t('team.refresh')}</Button></div>
    {(error || list.error) && <ErrorBanner reason={error || list.error || ''}>{t('team.failed')}</ErrorBanner>}
    {list.loading && <p role="status">{t('shell.loading')}</p>}
    {list.data && <><p className="muted text-xs mb-4">{t('team.inboxCount', { count: String(list.data.unread) })}</p>{!list.data.items.length ? <Surface><WorkbenchEmpty title={t('team.inboxEmpty')} description={t('team.inboxDescription')}/></Surface> : <div className="team-inbox-list">{list.data.items.map(item => <article key={item.id} data-inbox-id={item.id} className="team-inbox-row" data-unread={!item.read}><span className="team-inbox-dot" aria-hidden/><span className="team-inbox-category"><Badge>{categories[item.category]}</Badge></span><span className="team-inbox-time"><TimeAgo iso={item.at}/></span><Link className="team-inbox-title" to={item.category === 'monitor' ? `/status?project=${item.projectId}&id=${item.monitorId}` : `/tasks?project=${item.projectId}&id=${item.taskId}`}>{item.title}</Link><p className="team-inbox-body">{item.category === 'monitor' ? (item.body === 'down' ? t('monitor.down') : t('monitor.recovered')) : item.category === 'task' ? stages[item.body] ?? item.body : item.body}</p><div className="team-row-actions"><Button small className="button-icon" variant="ghost" disabled={busy} aria-label={item.read ? t('team.markUnread') : t('team.markRead')} onClick={() => void update(item.id, { read: !item.read })}>{item.read ? <Mail size={15}/> : <Check size={15}/>}</Button><Button small className="button-icon" variant="ghost" disabled={busy} aria-label={item.archived ? t('team.restore') : t('team.archive')} onClick={() => void update(item.id, { archived: !item.archived })}><Archive size={15}/></Button></div></article>)}</div>}<div className="flex gap-2 mt-4"><Button small disabled={!offset} onClick={() => set('offset', String(Math.max(0, offset - 50)))}>{t('queue.previous')}</Button><Button small disabled={list.data.nextOffset === null} onClick={() => set('offset', String(list.data!.nextOffset))}>{t('queue.next')}</Button></div></>}
    {prefsOpen && <Preferences onClose={() => setPrefsOpen(false)}/>}
  </div>;
}
function Preferences({ onClose }: { onClose: () => void }) {
  const t = useT(), prefs = useScopedAsyncData('preferences', () => api.inboxPreferences());
  const [draft, setDraft] = useState<InboxPreferences | null>(null), [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null);
  const current = draft ?? prefs.data, lock = useRef(false);
  async function save() {
    if (!current || lock.current) return;
    lock.current = true; setBusy(true); setError(null);
    try { await api.saveInboxPreferences(current); onClose(); } catch (failure) { setError(formatApiError(failure)); }
    finally { lock.current = false; setBusy(false); }
  }
  const labels = { assignments: t('team.assignment'), mentions: t('team.mention'), taskUpdates: t('team.taskUpdates'), monitors: t('team.monitors') };
  return <Dialog open onClose={onClose} closeLabel={t('common.close')} title={t('team.preferences')} footer={<Button variant="primary" busy={busy} disabled={!current} onClick={() => void save()}>{t('team.save')}</Button>}><div className="form-stack">{(error || prefs.error) && <ErrorBanner reason={error || prefs.error || ''}>{t('team.failed')}</ErrorBanner>}{prefs.error && <Button onClick={prefs.reload}>{t('common.retry')}</Button>}{current && (Object.keys(labels) as (keyof InboxPreferences)[]).map(key => <Checkbox key={key} checked={current[key]} disabled={busy} onChange={nextChecked => setDraft({ ...current, [key]: nextChecked })} label={labels[key]}/>)}</div></Dialog>;
}
