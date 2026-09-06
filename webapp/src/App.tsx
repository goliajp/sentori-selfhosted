import {
  Activity,
  FolderKanban,
  Inbox,
  Moon,
  Monitor,
  Package,
  Search,
  Send,
  Settings,
  Sun,
} from 'lucide-react';
import { createContext, useContext, useEffect, useState } from 'react';
import { NavLink, Outlet } from 'react-router-dom';

import { CommandPalette, openPalette } from './components/CommandPalette';
import { Kbd } from './components/ui';
import { useT } from './i18n';
import { api, type Me, type Project } from './lib/api';
import { formatApiError } from './lib/useAsyncData';
import { setThemeMode, useThemeMode, type ThemeMode } from './lib/theme';

import { SentoriMark } from './components/brand';

/// Shell context: who is logged in + which projects are visible.
/// Pages read the project scope from here instead of re-fetching.
export type Shell = {
  me: Me;
  projects: Project[];
  reloadProjects: () => void;
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
  const [me, setMe] = useState<Me | null>(null);
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
    api.authMe().then(setMe, (e: unknown) => setBootError(formatApiError(e)));
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
    void runProjects();
  };

  useEffect(() => {
    void runAuth();
  }, []);
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
  const activeProject =
    projects.find((p) => p.id === projectId) ?? projects[0] ?? null;
  const setActiveProjectId = (id: string) => {
    setProjectId(id);
    try {
      localStorage.setItem('sentori_project', id);
    } catch {
      // storage may be unavailable; scope still works for the session
    }
  };

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
        ? 'bg-raised font-medium text-fg'
        : 'text-fg-muted hover:bg-raised/60 hover:text-fg'
    }`;
  const navIcon = 'h-4 w-4 shrink-0 text-fg-subtle';

  return (
    <ShellContext.Provider
      value={{ me, projects, reloadProjects, activeProject, setActiveProjectId }}
    >
      {/* The app owns the viewport absolutely: the page never
          scrolls; every scroll area is an explicit interior region. */}
      <div className="fixed inset-0 flex flex-col bg-canvas">
      <div className="flex min-h-0 flex-1">
        <aside className="flex w-48 shrink-0 flex-col border-r border-border bg-bg p-2.5">
          <div className="mb-5 flex items-center gap-2 px-2.5 pt-1.5">
            <SentoriMark className="h-4.5 w-4.5 text-fg" />
            <span className="text-[15px] font-semibold tracking-tight">sentori</span>
          </div>
          <nav className="flex flex-col gap-0.5">
            <NavLink to="/projects" className={navCls}>
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
                <NavLink to="/" end className={navCls}>
                  <Inbox className={navIcon} aria-hidden />
                  {t('nav.inbox')}
                </NavLink>
                <NavLink to="/instruments" className={navCls}>
                  <Activity className={navIcon} aria-hidden />
                  {t('nav.instruments')}
                </NavLink>
                <NavLink to="/releases" className={navCls}>
                  <Package className={navIcon} aria-hidden />
                  {t('nav.releases')}
                </NavLink>
                {/* Its own entry, not a settings tab. Whether last
                    night's alert reached anyone is not the same
                    question as what your password is. */}
                <NavLink to="/push" className={navCls}>
                  <Send className={navIcon} aria-hidden />
                  {t('nav.push')}
                </NavLink>
              </>
            )}
            <div className="mt-3" />
            <NavLink to="/settings" className={navCls}>
              <Settings className={navIcon} aria-hidden />
              {t('nav.settings')}
            </NavLink>
          </nav>
          <div className="mt-auto px-1 pb-1">
            <ThemeSwitch />
          </div>
        </aside>

        <div className="flex min-w-0 flex-1 flex-col">
          <header className="flex h-11 shrink-0 items-center gap-3 border-b border-border bg-bg px-4">
            <button
              type="button"
              onClick={openPalette}
              className="flex h-7 w-72 items-center gap-2 rounded-md border border-border bg-surface px-2.5 text-sm text-fg-subtle transition-colors hover:border-border-strong hover:text-fg-muted"
            >
              <Search aria-hidden className="h-3.5 w-3.5 shrink-0" />
              <span className="min-w-0 flex-1 truncate text-left">
                {t('palette.placeholder')}
              </span>
              <Kbd>⌘K</Kbd>
            </button>
            <div className="ml-auto flex items-center gap-3">
              <span className="text-xs text-fg-subtle">
                {me.email}
                <span className="ml-1.5 text-fg-subtle/70">
                  {me.role === 'superadmin' ? t('shell.roleOwner') : t('shell.roleAdmin')}
                </span>
              </span>
            </div>
          </header>
          <main className="min-h-0 flex-1 overflow-hidden">
            <Outlet />
          </main>
        </div>
      </div>
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
    <footer className="flex h-6 shrink-0 items-center gap-2 border-t border-border bg-bg px-3 font-mono text-xs text-fg-subtle">
      <SentoriMark className="h-3 w-3 opacity-80" />
      <span>GOLIA · sentori</span>
      <span className="ml-auto flex items-center gap-3 tabular-nums">
        <span>webapp v{__APP_VERSION__}</span>
        {server && <span>server v{server}</span>}
      </span>
    </footer>
  );
}

/// Three-state theme switch — a segmented row that reads as
/// furniture, not as a feature. State + persistence live in
/// lib/theme.ts.
function ThemeSwitch() {
  const t = useT();
  const mode = useThemeMode();
  const setMode = setThemeMode;
  const options: {
    mode: ThemeMode;
    label: string;
    Glyph: typeof Monitor;
  }[] = [
    { mode: 'system', label: t('theme.system'), Glyph: Monitor },
    { mode: 'light', label: t('theme.light'), Glyph: Sun },
    { mode: 'dark', label: t('theme.dark'), Glyph: Moon },
  ];
  return (
    <div
      role="radiogroup"
      aria-label={t('theme.label')}
      className="flex rounded-md border border-border p-0.5"
    >
      {options.map((o) => (
        <button
          key={o.mode}
          type="button"
          role="radio"
          aria-checked={mode === o.mode}
          title={o.label}
          aria-label={o.label}
          onClick={() => setMode(o.mode)}
          className={`flex flex-1 items-center justify-center rounded py-1 transition-colors ${
            mode === o.mode
              ? 'bg-raised text-fg'
              : 'text-fg-subtle hover:text-fg-muted'
          }`}
        >
          <o.Glyph aria-hidden className="h-3.5 w-3.5" />
        </button>
      ))}
    </div>
  );
}
