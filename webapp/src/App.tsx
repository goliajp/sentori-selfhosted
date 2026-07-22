import { useThemeEffect } from '@goliapkg/gds/systems';
import { useEffect, useState } from 'react';
import { NavLink, Outlet, useNavigate, useParams } from 'react-router-dom';

import { CommandPalette } from './components/CommandPalette';
import { QuickPrefs } from './components/QuickPrefs';
import { WorkspaceSwitcher } from './components/WorkspaceSwitcher';
import { useT } from './i18n';
import { api, MeResponse } from './lib/api';
import { useNavShortcuts } from './lib/useShortcuts';

/// Main app shell — sidebar + content outlet. Wraps every
/// authenticated page.
export function App() {
  const [verified, setVerified] = useState(false);
  const [me, setMe] = useState<MeResponse | null>(null);
  const [paletteOpen, setPaletteOpen] = useState(false);
  // GDS repaints the --gds-* custom properties whenever the theme
  // atom changes; without this the toggle updates state and nothing
  // on screen moves.
  useThemeEffect();
  useNavShortcuts();

  // Global Cmd-K / Ctrl-K to toggle the command palette.
  // Also '?' to jump to /shortcuts cheatsheet.
  const navigate = useNavigate();
  useEffect(() => {
    function inEditable(): boolean {
      const a = document.activeElement;
      if (!a) return false;
      const tag = a.tagName;
      if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true;
      if ((a as HTMLElement).isContentEditable) return true;
      return false;
    }
    function onKey(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault();
        setPaletteOpen(o => !o);
        return;
      }
      if (e.key === '?' && !inEditable() && !e.metaKey && !e.ctrlKey) {
        e.preventDefault();
        navigate('/shortcuts');
      }
    }
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [navigate]);

  // Boot-time session probe. If the cookie + bearer header are
  // missing or invalid, api.authMe() throws and the 401 handler
  // in api.ts kicks the user to /login. We just track verified
  // so the shell doesn't flicker the dashboard before redirect.
  useEffect(() => {
    api
      .authMe()
      .then(me => {
        if (me?.user_id) {
          setMe(me);
          // Stash UI display fields so the rest of the app can
          // show them without hitting /auth/me on every render.
          try {
            localStorage.setItem('sentori_user_id', me.user_id);
            localStorage.setItem('sentori_email', me.email);
          } catch {
            // Storage disabled (private mode / quota) — display
            // fields just aren't cached; /auth/me stays the source.
          }
        }
        setVerified(true);
      })
      .catch(() => {
        // 401 handler will redirect — leave verified=false so the
        // shell renders a neutral spinner until the redirect lands.
      });
  }, []);

  if (!verified) {
    return (
      <div className="flex h-screen items-center justify-center bg-bg text-sm text-fg-subtle">
        Loading…
      </div>
    );
  }
  return (
    <div className="flex h-screen">
      <Sidebar me={me} />
      {/* The shell owns page padding and the measure. Pages used to
          each decide for themselves, and half of them forgot — issue
          detail, members, projects, metrics and others rendered flush
          against the viewport edge, clipping their own header actions
          on a wide display. One container here, none in the pages. */}
      <main className="flex-1 overflow-y-auto bg-canvas">
        <div className="mx-auto w-full max-w-[1600px] px-8 py-8">
          <Outlet />
        </div>
      </main>
      <CommandPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
      />
    </div>
  );
}

function Sidebar({ me }: { me: MeResponse | null }) {
  // Sidebar layout mirrors the lens grouping from legacy
  // `web/src/modules/registry.tsx`: workspace-wide pages
  // up top, per-project pages picked when a project is
  // selected.
  const t = useT();
  const params = useParams<{ id?: string }>();
  const projectScoped = params.id;

  return (
    <aside className="flex h-full w-56 shrink-0 flex-col overflow-hidden border-r border-border bg-bg px-5 py-4">
      <div className="mb-4">
        <h1 className="text-base font-semibold tracking-tight text-fg">
          Sentori
        </h1>
        <p className="font-mono text-xs text-fg-subtle">v0.2</p>
      </div>

      {/* Active workspace + switcher. Hidden until whoami resolves. */}
      {me && <WorkspaceSwitcher me={me} />}

      {/* The nav scrolls, the account row below it does not. Without
          this the list simply overflowed the aside on a short window:
          its background and right border stopped at the fold while the
          last few destinations carried on past them, unreachable. */}
      <nav className="mt-4 flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto text-sm">
        <SectionLabel>{t('nav.sectionWorkspace')}</SectionLabel>
        <NavItem to="/main" label={t('nav.overview')} />
        <NavItem to="/search" label={t('nav.search')} />
        <NavItem to="/projects" label={t('nav.projects')} />
        <NavItem to="/members" label={t('nav.members')} />
        <NavItem to="/alerts" label={t('nav.alerts')} />
        <NavItem to="/saved-views" label={t('nav.savedViews')} />
        <NotificationsNavItem />
        <NavItem to="/audit" label={t('nav.audit')} />
        <NavItem to="/settings/billing" label={t('nav.billing')} />
        <NavItem to="/settings" label={t('nav.settings')} />
        <NavItem to="/health" label={t('nav.health')} />
        {/* Cross-workspace operator surface — only for saasadmins.
            The route is server-gated too; this hides the entry. */}
        {me?.is_saasadmin && <NavItem to="/saas" label={t('nav.saasAdmin')} />}

        {projectScoped && (
          <>
            <SectionLabel className="mt-6">{t('nav.sectionProject')}</SectionLabel>
            <NavItem to={`/projects/${projectScoped}/issues`} label={t('nav.issues')} />
            <NavItem to={`/projects/${projectScoped}/events`} label={t('nav.events')} />
            <NavItem to={`/projects/${projectScoped}/traces`} label={t('nav.traces')} />
            <NavItem
              to={`/projects/${projectScoped}/track`}
              label={t('nav.track')}
            />
            <NavItem
              to={`/projects/${projectScoped}/metrics`}
              label={t('nav.metrics')}
            />
            <NavItem
              to={`/projects/${projectScoped}/replays`}
              label={t('nav.replays')}
            />
            <NavItem
              to={`/projects/${projectScoped}/cert`}
              label={t('nav.cert')}
            />
            <NavItem
              to={`/projects/${projectScoped}/probes`}
              label={t('nav.probes')}
            />
            <NavItem to={`/projects/${projectScoped}/tokens`} label={t('nav.tokens')} />
            <NavItem to={`/projects/${projectScoped}/push`} label={t('nav.push')} />
            <NavItem
              to={`/projects/${projectScoped}/integrations`}
              label={t('nav.integrations')}
            />
            <NavItem
              to={`/projects/${projectScoped}/releases`}
              label={t('nav.releases')}
            />
          </>
        )}
      </nav>

      <UserFooter />
    </aside>
  );
}

function NotificationsNavItem() {
  const t = useT();
  const [unread, setUnread] = useState(0);

  useEffect(() => {
    api
      .listNotifications()
      .then(r => setUnread(r.unread))
      .catch(() => {});
    const id = setInterval(() => {
      api
        .listNotifications()
        .then(r => setUnread(r.unread))
        .catch(() => {});
    }, 60_000);
    return () => clearInterval(id);
  }, []);

  return (
    <NavLink
      to="/notifications"
      end
      className={({ isActive }) =>
        `flex items-center justify-between rounded px-2.5 py-1.5 transition ${
          isActive
            ? 'bg-raised text-fg'
            : 'text-fg-muted hover:bg-surface hover:text-fg'
        }`
      }
    >
      <span>{t('nav.inbox')}</span>
      {unread > 0 && (
        <span className="rounded bg-ok/15 px-1.5 py-0.5 text-xs font-mono text-ok">
          {unread}
        </span>
      )}
    </NavLink>
  );
}

function UserFooter() {
  const t = useT();
  const navigate = useNavigate();
  const email =
    typeof localStorage !== 'undefined'
      ? localStorage.getItem('sentori_email')
      : null;

  async function signOut() {
    try {
      await fetch('/auth/logout', {
        method: 'POST',
        credentials: 'include',
      });
    } catch {
      // Best-effort: the cookie expires server-side anyway, and
      // local state is cleared below regardless.
    }
    try {
      localStorage.removeItem('sentori_user_id');
      localStorage.removeItem('sentori_email');
    } catch {
      // Storage disabled — nothing was cached to clear.
    }
    navigate('/login');
  }

  return (
    <div className="mt-4 space-y-2 border-t border-border pt-3 text-xs">
      {/* Theme + language sit above the account row: they are used far
          more often than sign-out, and burying them in Settings meant
          nobody found them. */}
      <QuickPrefs />
      {email && (
      <div className="flex items-center justify-between gap-1">
        <span
          className="truncate font-mono text-fg-muted"
          title={email}
        >
          {email}
        </span>
        <button
          onClick={signOut}
          title={t('action.signOut')}
          className="inline-flex h-7 w-7 items-center justify-center rounded text-fg-subtle transition hover:bg-raised hover:text-fg focus-visible:outline focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent"
        >
          ⎋
        </button>
      </div>
      )}
    </div>
  );
}

function NavItem({ to, label }: { to: string; label: string }) {
  return (
    <NavLink
      to={to}
      end
      className={({ isActive }) =>
        `rounded px-2.5 py-1.5 transition ${
          isActive
            ? 'bg-raised text-fg'
            : 'text-fg-muted hover:bg-surface hover:text-fg'
        }`
      }
    >
      {label}
    </NavLink>
  );
}

function SectionLabel({
  children,
  className = '',
}: {
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <p
      className={`mb-1 px-2.5 text-xs font-medium uppercase tracking-wider text-fg-subtle ${className}`}
    >
      {children}
    </p>
  );
}
