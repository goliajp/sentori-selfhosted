import { useReleaseNames } from '../lib/useReleaseNames';
import { useDocumentSearchParams } from '../lib/search-params';
import { Metric } from '../../shared/design/Metric';
import { WorkbenchEmpty } from '../components/WorkbenchEmpty';
import { ExternalTask } from '../components/IntegrationDelivery';
import { TaskIntegration } from '../components/TaskIntegration';
import { useId, useRef, useState } from 'react';
import { Link, useSearchParams } from 'react-router-dom';
import { Plus, ArrowLeft } from 'lucide-react';
import { Heading, Surface } from '../../shared/design/Surface';
import { Badge, Button, Field, Input, Textarea } from '../../shared/design/controls';
import { Dialog } from '../../shared/design/Dialog';
import { SegmentedControl } from '../../shared/design/SegmentedControl';
import { useShell } from '../App';
import { ErrorBanner, Select as NativeSelect, TimeAgo } from '../components/ui';
import { useT } from '../i18n';
import { api, ApiError } from '../lib/api';
import { formatApiError } from '../lib/useAsyncData';
import { useScopedAsyncData } from '../lib/useScopedAsyncData';
import { commandId, nextStages, taskStages, type Task, type TaskCommand, type TaskFields, type TaskStage } from '../lib/team-contract';

function useLabels() {
  const t = useT();
  return { triage: t('team.triage'), ready: t('team.ready'), inProgress: t('team.inProgress'), review: t('team.review'), verify: t('team.verify'), done: t('team.done'), cancelled: t('team.cancelled') };
}
function useCommand(project: string, saved: (task: Task) => void) {
  const [busy, setBusy] = useState(false), [error, setError] = useState<string | null>(null), [uncertain, setUncertain] = useState(false);
  const lock = useRef(false), pending = useRef<TaskCommand | null>(null);
  async function run(command: TaskCommand) {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError(null);
    pending.current ??= command;
    try { const task = await api.taskCommand(project, pending.current); pending.current = null; setUncertain(false); saved(task); }
    catch (failure) {
      if (failure instanceof ApiError && failure.status >= 400 && failure.status < 500) { pending.current = null; setUncertain(false); }
      else setUncertain(true);
      setError(formatApiError(failure));
    } finally { lock.current = false; setBusy(false); }
  }
  return { busy, error, uncertain, run, retry: () => { if (pending.current) void run(pending.current); } };
}
export default function TasksPage() {
  const { activeProject } = useShell(), t = useT();
  return activeProject ? <Tasks key={activeProject.id} project={activeProject.id}/> : <div className="production-page"><Heading title={t('team.title')}/><Surface><WorkbenchEmpty title={t('team.chooseProject')} action={<Link className="text-link" to="/projects">{t('nav.projects')}</Link>}/></Surface></div>;
}
function Tasks({ project }: { project: string }) {
  const currentSearchParams=useDocumentSearchParams();
  const t = useT(), labels = useLabels();
  const [params, setParams] = useSearchParams(), [editingTask, setEditingTask] = useState<Task | null>(null);
  const editor = params.has('new') ? 'new' : editingTask;
  const rawOffset=Number(params.get('offset')??0), offset=Number.isSafeInteger(rawOffset)&&rawOffset>=0?rawOffset:0;
  const query = new URLSearchParams({offset:String(offset)});
  for (const key of ['q', 'stage', 'mine']) if (params.get(key)) query.set(key, params.get(key)!);
  if (params.get('issue')) query.set('issueId', params.get('issue')!);
  const list = useScopedAsyncData(`${project}:${query}`, () => api.listTasks(project, query));
  const id = params.get('id'), view = params.get('view') === 'board' ? 'board' : 'list';
  const backParams=new URLSearchParams(params);backParams.delete('id');backParams.delete('new');
  const taskUrl=(id:string)=>{const next=new URLSearchParams(params);next.set('project',project);next.set('id',id);next.delete('new');return {pathname:'/tasks',search:next.toString()};};
  const members = list.data?.members ?? [];
  const set = (key: string, value: string) => { const next = currentSearchParams(); if (value) next.set(key, value); else next.delete(key); if (key !== 'offset') next.delete('offset'); setParams(next); };
  const saved = (task: Task) => { setEditingTask(null); const next = currentSearchParams(); next.set('id', task.id); next.delete('new'); next.delete('issue'); setParams(next); list.reload(); };
  const card = (task: Task) => <Link key={task.id} data-task-id={task.id} className="team-task-card" to={taskUrl(task.id)}><div><span className="team-task-id">{task.id.slice(0, 8)}</span><Badge tone={task.stage === 'done' ? 'success' : 'neutral'}>{labels[task.stage]}</Badge></div><strong>{task.title}</strong><footer><span>{task.priority === 'urgent' ? t('team.urgent') : task.priority === 'high' ? t('team.high') : task.priority === 'low' ? t('team.low') : t('team.normal')}</span><span>{members.find(m => m.id === task.assignee)?.name ?? (task.assignee ? t('team.formerAssignee') : t('team.unassigned'))}</span></footer></Link>;
  return <div className="production-page team-page">
    {id ? <TaskDetail key={id} project={project} id={id} members={members} onEdit={setEditingTask} backSearch={backParams.toString()}/> : <>
      <Heading title={t('team.title')} description={t('team.description')} action={<Button variant="primary" onClick={() => set('new','task')}><Plus size={15}/>{t('team.create')}</Button>}/>
      <div className="metrics-grid mb-6" aria-busy={list.loading}>
        <Metric label={t('workbench.activeTasks')} value={list.data?.summary?.active??'—'} note={t('workbench.filteredTasks')}/>
        <Metric label={t('workbench.reviewTasks')} value={list.data?.summary?.review??'—'}/>
        <Metric label={t('workbench.doneTasks')} value={list.data?.summary?.done??'—'}/>
        <Metric label={t('workbench.overdueTasks')} value={list.data?.summary?.overdue??'—'}/>
      </div>
      <div className="surface workbench-toolbar"><div className="team-toolbar">
        <Input type="search" aria-label={t('team.search')} placeholder={t('team.search')} value={params.get('q') ?? ''} onChange={e => set('q', e.target.value)}/>
        <Select label={t('team.assignee')} value={params.get('mine') ?? ''} onChange={v => set('mine', v)} options={[['', t('team.everyone')], ['true', t('team.mine')]]}/>
        <Select label={t('team.stage')} value={params.get('stage') ?? ''} onChange={v => set('stage', v)} options={[['', t('team.all')], ...taskStages.map(s => [s, labels[s]] as [string, string])]}/>
        <SegmentedControl label={t('team.view')} value={view} onChange={v => set('view', v)} options={[["list", t('team.list')], ["board", t('team.board')]]}/>
        <Button small onClick={list.reload}>{t('team.refresh')}</Button>
      </div></div>
      {list.error && <ErrorBanner reason={list.error}>{t('team.failed')}</ErrorBanner>}
      {list.loading && <p role="status">{t('shell.loading')}</p>}
      {list.data && <div className="surface task-results"><p className="surface-head muted text-xs">{t('team.pageHint', { count: String(list.data.total) })}</p>
        {list.data.tasks.length === 0 ? <WorkbenchEmpty kind="tasks" title={t('team.empty')} description={t('workbench.tasksEmptyHint')} action={<><Button variant="primary" onClick={()=>set('new','task')}><Plus size={15}/>{t('workbench.firstTask')}</Button>{(params.get('q')||params.get('stage')||params.get('mine')||params.get('issue')||offset>0)&&<Button onClick={()=>{const next=new URLSearchParams({project});setParams(next);}}>{t('projects.clearFilters')}</Button>}</>}/> : view === 'board' ? <div className="team-task-board" role="region" tabIndex={0} aria-label={t('team.board')}>{taskStages.map(s => <section key={s}><h2>{labels[s]} <span>{list.data!.tasks.filter(task => task.stage === s).length}</span></h2>{list.data!.tasks.filter(task => task.stage === s).map(card)}</section>)}</div> : <div className="team-task-list">{list.data.tasks.map(card)}</div>}
        <div className="table-pagination"><span>{t('queue.pageCount',{first:String(list.data.tasks.length?offset+1:0),last:String(list.data.tasks.length?offset+list.data.tasks.length:0),total:String(list.data.total)})}</span><div className="flex gap-2"><Button small disabled={list.loading||offset===0} onClick={() => set('offset', String(Math.max(0,offset-50)))}>{t('queue.previous')}</Button><Button small disabled={list.loading||list.data.nextOffset===null} onClick={() => set('offset', String(list.data!.nextOffset))}>{t('queue.next')}</Button></div></div>
      </div>}
    </>}
    {!id && <div className="workbench-section"><TaskIntegration key={project} project={project}/></div>}
    {editor && <TaskEditor key={editor === 'new' ? 'new' : `${editor.id}:${editor.version}`} project={project} task={editor === 'new' ? null : editor} issue={params.get('issue')} members={members} onSaved={saved} onClose={() => { setEditingTask(null); const next = currentSearchParams(); next.delete('new'); setParams(next, { replace: true }); }}/>}
  </div>;
}
function TaskEditor({ project, task, issue, members, onSaved, onClose }: { project: string; task: Task | null; issue: string | null; members: { id: string; name: string }[]; onSaved: (task: Task) => void; onClose: () => void }) {
  const t = useT();
  const [draft, setDraft] = useState<TaskFields>(() => task ? { title: task.title, description: task.description, kind: task.kind, priority: task.priority, assignee: task.assignee, due: task.due, reproduction: task.reproduction, rootCause: task.rootCause } : { title: '', description: '', kind: issue ? 'bug' : 'task', priority: 'normal', assignee: null, due: null, reproduction: '', rootCause: '' });
  const command = useCommand(project, onSaved), form = useId();
  function field<K extends keyof TaskFields>(key: K, value: TaskFields[K]) { setDraft(d => ({ ...d, [key]: value })); }
  return <Dialog open onClose={() => { if (!command.busy && !command.uncertain) onClose(); }} title={task ? t('team.edit') : t('team.create')} closeLabel={t('common.close')} footer={<><Button disabled={command.busy || command.uncertain} onClick={onClose}>{t('team.cancel')}</Button><Button type="submit" form={form} variant="primary" busy={command.busy}>{command.uncertain ? t('common.retry') : t('team.save')}</Button></>}>
    <form id={form} className="form-stack" onSubmit={e => { e.preventDefault(); void command.run(task ? { action: 'edit', commandId: commandId(), taskId: task.id, expectedVersion: task.version, fields: draft } : { action: 'create', commandId: commandId(), issueId: issue, fields: draft }); }}>
      <fieldset disabled={command.busy || command.uncertain} className="form-stack min-w-0">
        <Field label={t('team.taskTitle')}><Input required maxLength={200} value={draft.title} onChange={e => field('title', e.target.value)}/></Field>
        <div className="team-form-grid"><Select label={t('team.kind')} value={draft.kind} onChange={v => field('kind', v as TaskFields['kind'])} options={[["task", t('team.task')], ["bug", t('team.bug')]]}/><Select label={t('team.priority')} value={draft.priority} onChange={v => field('priority', v as TaskFields['priority'])} options={[["urgent", t('team.urgent')], ["high", t('team.high')], ["normal", t('team.normal')], ["low", t('team.low')]]}/><Select label={t('team.assignee')} value={draft.assignee ?? ''} onChange={v => field('assignee', v || null)} options={[["", t('team.unassigned')], ...members.map(m => [m.id, m.name] as [string, string])]}/><Field label={t('team.due')}><Input type="date" value={draft.due ?? ''} onChange={e => field('due', e.target.value || null)}/></Field></div>
        <Field label={t('team.details')}><Textarea value={draft.description} maxLength={16000} onChange={e => field('description', e.target.value)}/></Field>
        {draft.kind === 'bug' && <><Field label={t('team.reproduction')}><Textarea rows={3} value={draft.reproduction} maxLength={16000} onChange={e => field('reproduction', e.target.value)}/></Field><Field label={t('team.rootCause')}><Textarea rows={3} value={draft.rootCause} maxLength={16000} onChange={e => field('rootCause', e.target.value)}/></Field></>}
      </fieldset>
      {command.error && <ErrorBanner reason={command.error}>{t('team.failed')}</ErrorBanner>}
      {command.uncertain && <p role="status">{t('team.uncertain')}</p>}
      {task && command.error && !command.uncertain && <p>{t('team.conflict')}</p>}
    </form>
  </Dialog>;
}
function TaskDetail({ project, id, members, onEdit, backSearch }: { project: string; id: string; members: { id: string; name: string }[]; onEdit: (task: Task) => void; backSearch:string }) {
  const t = useT(), labels = useLabels();
  const [offset, setOffset] = useState(0), [revision, setRevision] = useState(0), [body, setBody] = useState(''), [mention, setMention] = useState('');
  const detail = useScopedAsyncData(`${project}:${id}:${offset}:${revision}`, () => api.getTask(project, id, offset));
  const command = useCommand(project, () => { setBody(''); setMention(''); setOffset(0); setRevision(v => v + 1); });
  const task = detail.data?.task;
  const events: Record<string, string> = { created: t('team.created'), updated: t('team.updated'), stage: t('team.stageChanged'), comment: t('team.commented'), 'issue.regressed': t('team.regressed'), 'auto.work.started': t('team.autoWork'), 'auto.pr.opened': t('team.autoPrOpened'), 'auto.pr.merged': t('team.autoPrMerged'), 'auto.ci.completed': t('team.autoChecks'), 'auto.release.deployed': t('team.autoDeployment') };
  return <>
    <Link className="team-back" to={{pathname:'/tasks',search:backSearch}}><ArrowLeft size={14}/>{t('team.title')}</Link>
    {detail.error && <ErrorBanner reason={detail.error}>{t('team.failed')}</ErrorBanner>}
    <div className="mb-4"><Button small onClick={detail.reload}>{t('team.refresh')}</Button></div>
    {task && <><Heading title={task.title} description={task.id} action={<Button onClick={() => onEdit(task)}>{t('team.edit')}</Button>}/>
      <div className="team-task-meta"><Badge tone={task.stage === 'done' ? 'success' : 'neutral'}>{labels[task.stage]}</Badge><span>{members.find(m => m.id === task.assignee)?.name ?? (task.assignee ? t('team.formerAssignee') : t('team.unassigned'))}</span>{task.due && <time dateTime={task.due}>{t('team.due')}: {task.due}</time>}{task.issueId && <Link to={`/issues/${task.issueId}?project=${project}`}>{t('team.issue')}</Link>}</div>
      <div className="team-stage-track" aria-label={t('team.stage')}>{taskStages.filter(s => s !== 'cancelled').map(s => <span key={s} data-active={s === task.stage}>{labels[s]}</span>)}</div>
      <details className="team-manual-stage"><summary>{t('team.manual')}</summary><p>{t('team.manualHint')}</p><div className="team-transition-actions">{nextStages[task.stage].map(stage => <Button key={stage} disabled={command.busy || command.uncertain} onClick={() => void command.run({ action: 'transition', commandId: commandId(), taskId: id, expectedVersion: task.version, stage })}>{labels[stage]}</Button>)}</div></details>
      {command.error && <ErrorBanner reason={command.error}>{t('team.failed')}</ErrorBanner>}
      {command.uncertain && <Button busy={command.busy} onClick={command.retry}>{t('common.retry')}</Button>}
      <div className="team-detail-grid"><div className="team-detail-main"><TaskEvidence task={task}/><Surface title={t('team.details')}><p className="team-text">{task.description || '—'}</p>{task.kind === 'bug' && <dl className="team-definition"><dt>{t('team.reproduction')}</dt><dd>{task.reproduction || '—'}</dd><dt>{t('team.rootCause')}</dt><dd>{task.rootCause || '—'}</dd></dl>}</Surface>
      <Surface title={t('team.activity')}><ol className="team-history">{detail.data!.events.map(event => <li key={event.id}><span className="team-history-dot"/><div><strong>{event.actorName ?? t('team.system')}</strong> {events[event.action] ?? event.action}{event.body && <p>{event.action === 'stage' ? labels[event.body as TaskStage] : event.body}</p>}<TimeAgo iso={event.at}/></div></li>)}</ol><div className="flex gap-2 mt-4"><Button small disabled={!offset} onClick={() => setOffset(Math.max(0, offset - 50))}>{t('queue.previous')}</Button><Button small disabled={detail.data!.nextOffset === null} onClick={() => setOffset(detail.data!.nextOffset!)}>{t('queue.next')}</Button></div>
        <form className="form-stack mt-4" onSubmit={e => { e.preventDefault(); void command.run({ action: 'comment', commandId: commandId(), taskId: id, expectedVersion: task.version, body, mention: mention || null }); }}><Field label={t('team.comment')}><Textarea required maxLength={16000} disabled={command.busy || command.uncertain} value={body} onChange={e => setBody(e.target.value)}/></Field><Select label={t('team.mention')} value={mention} onChange={setMention} options={[["", t('team.noMention')], ...members.map(m => [m.id, m.name] as [string, string])]}/><Button type="submit" variant="primary" disabled={!body.trim() || command.uncertain} busy={command.busy}>{t('team.post')}</Button></form>
      </Surface></div><aside><ExternalTask project={project} task={task}/></aside></div>
    </>}
  </>;
}

function Select({ label, value, onChange, options }: { label: string; value: string; onChange: (value: string) => void; options: [string, string][] }) {
  return <Field label={label}><NativeSelect value={value} onChange={e => onChange(e.target.value)}>{options.map(([id, name]) => <option key={id} value={id}>{name}</option>)}</NativeSelect></Field>;
}

function TaskEvidence({ task }: { task: Task }) {
  const t = useT();
  const evidence = task.automation as { workingCommit?: string; authority?: { repository?: string; number?: number; occurredAt?: string }; checks?: { unit?: string; e2e?: string; occurredAt?: string }; deployment?: { release?: string; environment?: string; occurredAt?: string } };
  const releases=useReleaseNames(task.projectId,[evidence.deployment?.release]);
  const outcome = (value?: string) => value === 'passed' ? t('team.passed') : value === 'failed' ? t('team.checkFailed') : '—';
  return <Surface title={t('team.evidence')}><p>{task.verified ? t('team.verified') : t('team.noEvidence')}</p>{evidence.workingCommit && <dl className="team-definition"><dt>{t('team.repository')}</dt><dd>{evidence.authority?.repository ?? '—'}</dd><dt>{t('team.commit')}</dt><dd><code>{evidence.workingCommit}</code></dd><dt>{t('team.pullRequest')}</dt><dd>{evidence.authority?.number ?? '—'}{evidence.authority?.occurredAt && <TimeAgo iso={evidence.authority.occurredAt}/>}</dd><dt>{t('team.unitChecks')}</dt><dd>{outcome(evidence.checks?.unit)}</dd><dt>{t('team.e2eChecks')}</dt><dd>{outcome(evidence.checks?.e2e)}{evidence.checks?.occurredAt && <TimeAgo iso={evidence.checks.occurredAt}/>}</dd><dt>{t('team.deployedRelease')}</dt><dd>{releases.label(evidence.deployment?.release)}{evidence.deployment?.occurredAt && <TimeAgo iso={evidence.deployment.occurredAt}/>}</dd><dt>{t('team.environment')}</dt><dd>{evidence.deployment?.environment ?? '—'}</dd></dl>}</Surface>;
}
