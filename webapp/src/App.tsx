import { WorkspaceLayout } from "../shared/design/WorkspaceLayout";
import {
  Activity,
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
import { createContext, lazy, useContext, useEffect, useRef, useState } from 'react';
import { Link, NavLink, Outlet, useLocation, useMatch, useSearchParams } from 'react-router-dom';

import { CommandPalette, openPalette } from './components/CommandPalette';
import { Kbd, ThemeSwitch } from './components/ui';
import { useT } from './i18n';
import { api, type Me, type Project } from './lib/api';
import { formatApiError, useAsyncData } from './lib/useAsyncData';

import { SentoriMark, SentoriIdentity } from './components/brand';
const Welcome = lazy(() => import('./pages/Welcome'));

/// Shell context: who is logged in + which projects are visible.
/// Pages read the project scope from here instead of re-fetching.
export type Shell = {
  me: Me;
  projects: Project[];
  reloadProjects: () => Promise<void>;
  /** The project the whole app is scoped to. */
  activeProject: Project | null;
  setActiveProjectId: (id: string) => void;
};

const ShellContext = createContext<Shell | null>(null);

export function useShell(): Shell {
  const s = useContext(ShellContext);
  if (!s) throw new Error('useShell outside <App>');
  return s;
}

/// App frame — this is an application, not a document: a fixed
/// viewport split into a nav rail, a slim topbar (search, theme,
/// identity) and a content region that panes scroll inside of. The
/// page never scrolls as a whole.
export function App() {
  const t = useT();
  const isPublicHome = useLocation().pathname === '/';
  const issueId = useMatch('/issues/:issueId')?.params.issueId;
  const [scopeParams, setScopeParams] = useSearchParams();
  const drawer = useRef<HTMLDialogElement>(null);
  const [me, setMe] = useState<Me | null>(null);
  const [visitor, setVisitor] = useState(false);
  const [projects, setProjects] = useState<Project[]>([]);
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
      (r) => setProjects(r.projects),
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
    const next = new URLSearchParams(scopeParams);
    next.set('project', activeProject.id);
    setScopeParams(next, { replace: true });
  }, [activeProject, scopeParams, setScopeParams]);
  const setActiveProjectId = (id: string) => {
    setProjectId(id);
    const next = new URLSearchParams(scopeParams);
    next.set('project', id);
    setScopeParams(next, { replace: true });
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
            <button
              type="button"
              className="text-fg-muted underline hover:text-fg"
              onClick={loadMe}
            >
              {t('common.retry')}
            </button>
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
            <NavLink to={scopedLink("/projects")}  className={navCls}>
              <FolderKanban className={navIcon} aria-hidden />
              {t('nav.projects')}
            </NavLink>
            {projectsError && (
              /* Without this the rail simply loses its project links and
                 reads as an instance with no projects yet. */
              <div className="mt-3 rounded border border-danger/40 px-2.5 py-2 text-xs text-danger">
                <div>{t('shell.projectsFailed')}</div>
                <button
                  type="button"
                  className="mt-1 underline"
                  onClick={reloadProjects}
                >
                  {t('common.retry')}
                </button>
              </div>
            )}
            {activeProject && (
              <>
                {/* the chosen project's own workspace */}
                <div className="mb-0.5 mt-3 truncate px-2.5 text-xs font-semibold text-fg-subtle">
                  {activeProject.name}
                </div>
                <NavLink to={scopedLink("/")}  end className={navCls}>
                  <Inbox className={navIcon} aria-hidden />
                  {t('nav.inbox')}
                </NavLink>
                <NavLink to={scopedLink("/tasks")} className={navCls}><ListTodo className={navIcon} aria-hidden/>{t('team.title')}</NavLink>
                <NavLink to={scopedLink("/status")} className={navCls}><Activity className={navIcon} aria-hidden/>{t('monitor.title')}</NavLink>
                <NavLink to={scopedLink("/integrations")} className={navCls}><Plug className={navIcon} aria-hidden/>{t('integration.title')}</NavLink>
                <NavLink to={scopedLink("/instruments")}  className={navCls}>
                  <Activity className={navIcon} aria-hidden />
                  {t('nav.instruments')}
                </NavLink>
                <NavLink to={scopedLink("/releases")}  className={navCls}>
                  <Package className={navIcon} aria-hidden />
                  {t('nav.releases')}
                </NavLink>
                <NavLink to={scopedLink("/setup")}  className={navCls}>
                  <Plug className={navIcon} aria-hidden />
                  {t('nav.setup')}
                </NavLink>
                {/* Its own entry, not a settings tab. Whether last
                    night's alert reached anyone is not the same
                    question as what your password is. */}
                <NavLink to={scopedLink("/push")}  className={navCls}>
                  <Send className={navIcon} aria-hidden />
                  {t('nav.push')}
                </NavLink>
              </>
            )}
            <div className="mt-3" />
            <NavLink to="/inbox" className={navCls}><Bell className={navIcon} aria-hidden/>{t('team.inbox')}</NavLink>
            {system.data && <NavLink to={system.data.deployment === 'cloud' ? '/settings/instance' : '/self-hosted'} className={navCls}><Server className={navIcon} aria-hidden/>{t(system.data.deployment === 'cloud' ? 'cloud.workspace' : 'lifecycle.title')}</NavLink>}
            <NavLink to={scopedLink("/settings")}  className={navCls}>
              <Settings className={navIcon} aria-hidden />
              {t('nav.settings')}
            </NavLink>
          </nav>
          <div className="mt-auto px-1 pb-1">
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
            <button type="button" onClick={() => drawer.current?.close()} aria-label={t('common.close')} className="mb-3 ml-auto grid h-10 w-10 place-items-center rounded-md hover:bg-raised">
              <X className="h-5 w-5" aria-hidden />
            </button>
            {navigation}
          </div>
        </dialog>

        <WorkspaceLayout workbench className="production-workspace" sidebar={navigation} toolbar={<>
            <button type="button" onClick={() => drawer.current?.showModal()} aria-label={t('shell.openNavigation')} className="grid h-10 w-10 shrink-0 place-items-center rounded-md hover:bg-raised workspace-mobile-toggle">
              <Menu className="h-5 w-5" aria-hidden />
            </button>
            <button
              type="button"
              onClick={openPalette}
              className="flex h-9 min-w-0 flex-1 items-center gap-2 rounded-md border border-border bg-surface px-2.5 text-sm text-fg-muted transition-colors hover:border-border-strong hover:text-fg sm:max-w-80"
            >
              <Search aria-hidden className="h-3.5 w-3.5 shrink-0" />
              <span className="min-w-0 flex-1 truncate text-left">
                {t('palette.placeholder')}
              </span>
              <Kbd>⌘K</Kbd>
            </button>
            <div className="ml-auto hidden items-center gap-3 sm:flex">
              <span className="text-xs text-fg-subtle">
                {me.email}
                <span className="ml-1.5 text-fg-subtle/70">
                  {me.role === 'superadmin' ? t('shell.roleOwner') : t('shell.roleAdmin')}
                </span>
              </span>
            </div>
          </>}>
          <main id="workspace" tabIndex={-1} className="min-h-0 flex-1 overflow-hidden outline-none">
            <Outlet />
          </main>
        </WorkspaceLayout>
      <StatusBar />
      <CommandPalette />
      </div>
    </ShellContext.Provider>
  );
}

/// The frame's ground line: who made this, which build is talking.
/// The webapp version is compiled in from package.json; the server
/// version comes from the instance actually answering.
function StatusBar() {
  const [server, setServer] = useState<string | null>(null);
  useEffect(() => {
    fetch('/healthz')
      .then((r) => r.json())
      .then((j: { version?: string }) => setServer(j.version ?? null))
      .catch(() => undefined);
  }, []);
  return (
    <footer className="flex h-6 shrink-0 items-center gap-2 overflow-hidden whitespace-nowrap border-t border-border bg-bg px-3 font-mono text-xs text-fg-subtle">
      <SentoriMark className="h-3 w-3 opacity-80" />
      <span><span className="hidden sm:inline">GOLIA · </span>sentori</span>
      <span className="ml-auto flex items-center gap-3 tabular-nums">
        <span>webapp v{__APP_VERSION__}</span>
        {server && <span>server v{server}</span>}
      </span>
    </footer>
  );
}
