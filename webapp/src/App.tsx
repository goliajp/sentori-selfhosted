import { Tooltip } from '../shared/design/Tooltip';
import { Action, ControlRow } from '../shared/design/controls';
import { currentSearchParams } from './lib/search-params';
import { WorkspaceLayout } from "../shared/design/WorkspaceLayout";
import {
  ChartNoAxesCombined,
  PanelsTopLeft,
  Radar,
  SquareTerminal,
  FolderKanban,
  Inbox,
  ListTodo,
  Bell,
  Menu,
  Package,
  Plug,
  Search,
  Send,
  Settings,
  Server,
  X,
} from 'lucide-react';
import { lazy, useEffect, useRef, useState } from 'react';
import { Link, NavLink, type NavLinkProps, useOutlet, useLocation, useMatch, useNavigate, useSearchParams } from 'react-router-dom';

import { CommandPalette, openPalette } from './components/CommandPalette';
import { Kbd, ThemeSwitch, ErrorBanner } from './components/ui';
import { LanguageSwitch } from './components/LanguageSwitch';
import { useT } from './i18n';
import { api, type Me, type Project } from './lib/api';
import { formatApiError, useAsyncData } from './lib/useAsyncData';

import { Dialog } from '../shared/design/Dialog';
import { Button } from '../shared/design/controls';
import type { SystemInfo } from './lib/system-contract';
import { SentoriIdentity } from './components/brand';
const Welcome = lazy(() => import('./pages/Welcome'));

export { useShell } from './lib/shell-context';
import { ShellContext, type ProjectDestination } from './lib/shell-context';
import { Workbench } from './components/Workbench';

function NavigationLink({ title, ...props }: NavLinkProps & { title: string }) {
  return <Tooltip label={title}><NavLink {...props}/></Tooltip>;
}

/// App frame — this is an application, not a document: a fixed
/// viewport split into a nav rail, a slim topbar (search, theme,
/// identity) and a content region that panes scroll inside of. The
/// page never scrolls as a whole.
export function App() {
  const t = useT();
  const outlet = useOutlet();
  const location=useLocation(),navigate=useNavigate();
  const isPublicHome = location.pathname === '/';
  const issueId = useMatch('/issues/:issueId')?.params.issueId;
  const [scopeParams, setScopeParams] = useSearchParams();
  const drawer = useRef<HTMLDialogElement>(null);
  const [me, setMe] = useState<Me | null>(null);
  const [visitor, setVisitor] = useState(false);
  const [projects, setProjects] = useState<Project[]>([]);
  const [projectsLoaded,setProjectsLoaded] = useState(false);
  // Both boot calls used to discard their failure with `() => undefined`,
  // and both failures then looked like ordinary states of a healthy app:
  // a failed authMe left `me` null forever, so every route rendered
  // "Loading…" and never stopped; a failed listProjects left the list
  // empty, so the rail lost its project and every page that scopes by
  // project fell through to its own empty state. A dashboard for
  // finding faults must not present its own as an absence of data.
  const [bootError, setBootError] = useState<null | string>(null);
  const [projectsError, setProjectsError] = useState<null | string>(null);

  // Clearing the previous error belongs to the retry button rather than
  // to these, so that neither sets state synchronously inside an effect.
  // A 401 inside api.authMe() redirects to /login, so anything arriving
  // here is a real failure and not a signed-out session.
  const runAuth = () =>
    (isPublicHome ? api.currentUser() : api.authMe()).then(
      (user) => { setMe(user); setVisitor(user === null); },
      (e: unknown) => setBootError(formatApiError(e)),
    );
  const runProjects = () =>
    api.listProjects().then(
      (r) => {setProjects(r.projects);setProjectsLoaded(true);},
      (e: unknown) => setProjectsError(formatApiError(e)),
    );

  const loadMe = () => {
    setBootError(null);
    void runAuth();
  };
  const reloadProjects = () => {
    setProjectsError(null);
    return runProjects();
  };

  useEffect(() => {
    void runAuth();
  }, [isPublicHome]);
  useEffect(() => {
    if (me) void runProjects();
  }, [me]);

  // Project scope: persisted, defaulting to the first project. The
  // whole app reads this — queue, instruments, releases.
  const [projectId, setProjectId] = useState<null | string>(() => {
    try {
      return localStorage.getItem('sentori_project');
    } catch {
      return null;
    }
  });
  // A legacy issue URL can arrive with another project selected locally.
  // Derive its scope from the authorized server response, then make it shareable.
  const { data: linkedIssue } = useAsyncData(
    () => me && issueId ? api.getIssue(issueId) : Promise.resolve(null),
    [me, issueId],
  );
  const system = useAsyncData(() => me ? api.systemInfo() : Promise.resolve(null), [me]);
  const linkedProject = linkedIssue?.id === issueId ? linkedIssue?.projectId : undefined;
  const requestedProject = linkedProject ?? scopeParams.get('project');
  const activeProject = requestedProject
    ? projects.find((p) => p.id === requestedProject) ?? null
    : projects.find((p) => p.id === projectId) ?? projects[0] ?? null;
  useEffect(() => {
    if (!activeProject || scopeParams.get('project') === activeProject.id) return;
    const next = currentSearchParams();
    next.set('project', activeProject.id);
    setScopeParams(next, { replace: true });
  }, [activeProject, scopeParams, setScopeParams]);
  const setActiveProjectId = (id: string, destination?: ProjectDestination) => {
    setProjectId(id);
    const next = destination ? new URLSearchParams() : currentSearchParams();
    if(!destination)for(const key of ['app','platform','appType','env','rel','release','ck','cv','dimension','context','tags','q','offset','occurrence','occurrenceOffset','detail','id','new','issue','job','transfer'])next.delete(key);
    if(id)next.set('project',id);else next.delete('project');
    for(const [name,value] of Object.entries(destination?.params??{}))next.set(name,value);
    // Project selection and its destination form one navigation. A second
    // navigation from the old document would correctly be scoped as inactive.
    if(destination)navigate({pathname:destination.pathname,search:next.toString()});
    else if(issueId){next.delete('tab');navigate({pathname:'/issues',search:next.toString()});}
    else setScopeParams(next, { replace: true });
    try {
      localStorage.setItem('sentori_project', id);
    } catch {
      // storage may be unavailable; scope still works for the session
    }
  };

  if (visitor && isPublicHome) return <Welcome />;
  if (!me) {
    return (
      <div className="flex h-screen flex-col items-center justify-center gap-3 bg-canvas text-sm">
        {bootError ? (
          <>
            <div className="text-danger">
              {t('shell.bootFailed')} {bootError}
            </div>
            <Action
              type="button"
              className="text-fg-muted underline hover:text-fg"
              onClick={loadMe}
            >
              {t('common.retry')}
            </Action>
          </>
        ) : (
          <span className="text-fg-subtle">{t('shell.loading')}</span>
        )}
      </div>
    );
  }

  const navCls = ({ isActive }: { isActive: boolean }) =>
    `flex items-center gap-2 rounded-md px-2.5 py-1.5 text-sm transition-colors ${
      isActive
        ? 'bg-selected text-selected-ink hover:bg-selected-hover'
        : 'text-fg-muted hover:bg-hover hover:text-fg'
    }`;
  const scopedLink = (pathname: string) => ({ pathname, search: activeProject ? new URLSearchParams({ project: activeProject.id }).toString() : '' });
  const navIcon = 'h-4 w-4 shrink-0 text-fg-subtle';

  const navigation = (
    <>
          <Link to="/welcome" aria-label="Sentori" className="sentori-identity mb-5 flex items-center px-2.5 py-2">
            <SentoriIdentity />
          </Link>

          <nav aria-label={t('shell.navigation')} className="flex flex-col gap-0.5">
            <NavigationLink to={scopedLink("/projects")}  className={navCls} aria-label={t('nav.projects')} title={t('nav.projects')}>
              <FolderKanban className={navIcon} aria-hidden />
              <span className="control-label-text">{t('nav.projects')}</span>
            </NavigationLink>
            {projectsError && (
              /* Without this the rail simply loses its project links and
                 reads as an instance with no projects yet. */
              <div className="mt-3 rounded border border-danger/40 px-2.5 py-2 text-xs text-danger">
                <div>{t('shell.projectsFailed')}</div>
                <Action
                  type="button"
                  className="mt-1 underline"
                  onClick={reloadProjects}
                >
                  {t('common.retry')}
                </Action>
              </div>
            )}
            {activeProject && (
              <>
                {/* the chosen project's own workspace */}
                <div className="mb-0.5 mt-3 truncate px-2.5 text-xs font-semibold text-fg-subtle">
                  {activeProject.name}
                </div>
                <NavigationLink to={scopedLink(location.pathname === "/" ? "/" : "/issues")} className={navCls} aria-label={t('nav.inbox')} title={t('nav.inbox')}>
                  <Inbox className={navIcon} aria-hidden />
                  <span className="control-label-text">{t('nav.inbox')}</span>
                </NavigationLink>
                <NavigationLink to={scopedLink("/tasks")} className={navCls} aria-label={t('team.title')} title={t('team.title')}><ListTodo className={navIcon} aria-hidden/><span className="control-label-text">{t('team.title')}</span></NavigationLink>
                <NavigationLink to={scopedLink("/web")} className={navCls} aria-label={t('web.title')} title={t('web.title')}><PanelsTopLeft className={navIcon} aria-hidden/><span className="control-label-text">{t('web.title')}</span></NavigationLink>
                <NavigationLink to={scopedLink("/status")} className={navCls} aria-label={t('monitor.title')} title={t('monitor.title')}><Radar className={navIcon} aria-hidden/><span className="control-label-text">{t('monitor.title')}</span></NavigationLink>
                <NavigationLink to={scopedLink("/integrations")} className={navCls} aria-label={t('integration.title')} title={t('integration.title')}><Plug className={navIcon} aria-hidden/><span className="control-label-text">{t('integration.title')}</span></NavigationLink>
                <NavigationLink to={scopedLink("/instruments")}  className={navCls} aria-label={t('nav.instruments')} title={t('nav.instruments')}>
                  <ChartNoAxesCombined className={navIcon} aria-hidden />
                  <span className="control-label-text">{t('nav.instruments')}</span>
                </NavigationLink>
                <NavigationLink to={scopedLink("/releases")}  className={navCls} aria-label={t('nav.releases')} title={t('nav.releases')}>
                  <Package className={navIcon} aria-hidden />
                  <span className="control-label-text">{t('nav.releases')}</span>
                </NavigationLink>
                <NavigationLink to={scopedLink("/setup")}  className={navCls} aria-label={t('nav.setup')} title={t('nav.setup')}>
                  <SquareTerminal className={navIcon} aria-hidden />
                  <span className="control-label-text">{t('nav.setup')}</span>
                </NavigationLink>
                {/* Its own entry, not a settings tab. Whether last
                    night's alert reached anyone is not the same
                    question as what your password is. */}
                <NavigationLink to={scopedLink("/push")}  className={navCls} aria-label={t('nav.push')} title={t('nav.push')}>
                  <Send className={navIcon} aria-hidden />
                  <span className="control-label-text">{t('nav.push')}</span>
                </NavigationLink>
              </>
            )}
            <div className="mt-3" />
            <NavigationLink to="/inbox" className={navCls} aria-label={t('team.inbox')} title={t('team.inbox')}><Bell className={navIcon} aria-hidden/><span className="control-label-text">{t('team.inbox')}</span></NavigationLink>
            {system.data && <NavigationLink to={system.data.deployment === 'cloud' ? '/settings/instance' : '/self-hosted'} className={navCls} aria-label={t(system.data.deployment === 'cloud' ? 'cloud.workspace' : 'lifecycle.title')} title={t(system.data.deployment === 'cloud' ? 'cloud.workspace' : 'lifecycle.title')}><Server className={navIcon} aria-hidden/><span className="control-label-text">{t(system.data.deployment === 'cloud' ? 'cloud.workspace' : 'lifecycle.title')}</span></NavigationLink>}
            <NavigationLink to={scopedLink("/settings")}  className={navCls} aria-label={t('nav.settings')} title={t('nav.settings')}>
              <Settings className={navIcon} aria-hidden />
              <span className="control-label-text">{t('nav.settings')}</span>
            </NavigationLink>
          </nav>
          <div className="mt-auto grid gap-2 px-1 pb-1">
            <div className="sm:hidden"><LanguageSwitch /></div>
            <ThemeSwitch />
          </div>
    </>
  );

  return (
    <ShellContext.Provider
      value={{ me, projects, reloadProjects, activeProject, setActiveProjectId }}
    >
      <div className="fixed inset-0 flex flex-col overflow-hidden bg-canvas">
      <a href="#workspace" className="sr-only z-50 rounded bg-accent p-3 text-accent-fg focus:not-sr-only focus:absolute">
        {t('shell.skip')}
      </a>
        <dialog
          ref={drawer}
          aria-label={t('shell.navigation')}
          className="fixed inset-y-0 left-0 right-auto m-0 h-dvh max-h-dvh w-72 max-w-[90vw] border-r border-border bg-bg p-4 text-fg backdrop:bg-black/50"
          onClick={(e) => {
            if (e.target === e.currentTarget || (e.target as HTMLElement).closest('a')) drawer.current?.close();
          }}
        >
          <div className="flex h-full flex-col">
            <Action type="button" onClick={() => drawer.current?.close()} aria-label={t('common.close')} className="mb-3 ml-auto grid h-10 w-10 place-items-center rounded-md hover:bg-raised">
              <X className="h-5 w-5" aria-hidden />
            </Action>
            {navigation}
          </div>
        </dialog>

        <WorkspaceLayout scope="product" workbench className="production-workspace" sidebar={<><div className="production-rail-links">{navigation}</div></>} toolbar={<>
            <Link to="/welcome" aria-label="Sentori" className="sentori-identity sentori-topbar-identity"><SentoriIdentity /></Link>
            <Action type="button" onClick={() => drawer.current?.showModal()} aria-label={t('shell.openNavigation')} className="grid h-10 w-10 shrink-0 place-items-center rounded-md hover:bg-raised workspace-mobile-toggle">
              <Menu className="h-5 w-5" aria-hidden />
            </Action>
            <Action
              type="button"
              onClick={openPalette}
              className="flex h-9 min-w-0 flex-1 items-center gap-2 rounded-md border border-border bg-surface px-2.5 text-sm text-fg-muted transition-colors hover:border-border-strong hover:text-fg sm:max-w-80"
            >
              <Search aria-hidden className="h-3.5 w-3.5 shrink-0" />
              <span className="min-w-0 flex-1 truncate text-left">
                {t('palette.placeholder')}
              </span>
              <Kbd>⌘K</Kbd>
            </Action>
            <div className="ml-auto hidden items-center gap-3 sm:flex">
              <ControlRow size="small" layout="content">
                <LanguageSwitch />
                <ThemeSwitch />
              </ControlRow>
              <span className="text-xs text-fg-subtle">
                {me.email}
                <span className="ml-1.5 text-fg-subtle/70">
                  {me.role === 'superadmin' ? t('shell.roleOwner') : t('shell.roleAdmin')}
                </span>
              </span>
            </div>
          </>}>
          <main id="workspace" tabIndex={-1} className="flex min-h-0 flex-1 overflow-hidden outline-none">
            {projectsError?<div className="p-6"><ErrorBanner reason={projectsError}>{t('shell.projectsFailed')}</ErrorBanner><Action type="button" className="button" onClick={reloadProjects}>{t('common.retry')}</Action></div>:!projectsLoaded?<p role="status" className="p-6 muted">{t('shell.loading')}</p>:<Workbench>{outlet}</Workbench>}
          </main>
        </WorkspaceLayout>
      <StatusBar system={system.data} systemError={system.error} retry={system.reload}/>
      <CommandPalette />
      </div>
    </ShellContext.Provider>
  );
}

/** Deployment identity comes from this instance, independently of the signed-in user. */
function StatusBar({ system, systemError, retry }: { system:SystemInfo|null; systemError:string|null; retry:()=>void }) {
  const t=useT(), [open,setOpen]=useState(false);
  const instance=useAsyncData(()=>api.instanceInfo(),[]);
  const reloadInstance=instance.reload;
  useEffect(()=>api.onChange(path=>{if(path.startsWith('/admin/api/instance'))reloadInstance();}),[reloadInstance]);
  const operator=system?.operatorName;
  const identity=system ? system.deployment==='cloud'
    ? operator ? t('shell.saasBy',{name:operator}) : t('shell.saas')
    : operator ? t('shell.selfHostedBy',{operator,name:instance.data?.name || window.location.host}) : t('shell.selfHostedInstance',{name:instance.data?.name || window.location.host}) : t('shell.instanceUnknown');
  return <>
    <footer className="instance-statusbar">
      <Action type="button" className="instance-status-identity" onClick={()=>setOpen(true)} title={t('shell.instanceDetails')}><Server size={12} aria-hidden/><span className="control-label-text">{identity}</span></Action>
      <Action type="button" className="instance-status-version" onClick={()=>setOpen(true)} aria-label={t('shell.instanceDetails')}>
        <span>{t('shell.interfaceVersion',{version:__APP_VERSION__})}</span>
        {system&&<span>{t('shell.serviceVersion',{version:system.serverVersion})}</span>}
      </Action>
    </footer>
    <Dialog open={open} title={t('shell.instanceDetails')} closeLabel={t('common.close')} onClose={()=>setOpen(false)}>
      <dl className="instance-status-details">
        <dt>{t('shell.deploymentMode')}</dt><dd>{system?t(system.deployment==='cloud'?'shell.saas':'shell.selfHosted'):t('shell.instanceUnknown')}</dd>
        <dt>{t('shell.operator')}</dt><dd>{operator||t('shell.operatorUnknown')}</dd>
        <dt>{t('instance.name')}</dt><dd>{instance.data?.name||'—'}</dd>
        <dt>{t('instance.origin')}</dt><dd>{instance.data?.origin||window.location.origin}</dd>
        <dt>{t('instance.id')}</dt><dd>{instance.data?.id||'—'}</dd>
        <dt>{t('shell.interface')}</dt><dd>{__APP_VERSION__}</dd>
        <dt>{t('shell.service')}</dt><dd>{system?.serverVersion||'—'}</dd>
      </dl>
      {(systemError||instance.error)&&<div role="alert"><p>{systemError||instance.error}</p><Button onClick={()=>{retry();instance.reload();}}>{t('common.retry')}</Button></div>}
      <p className="muted text-sm mt-4">{t('shell.productCredit')}</p>
    </Dialog>
  </>;
}
