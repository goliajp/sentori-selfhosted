import { useEffect, useState } from 'react';
import { NavLink, Outlet, useNavigate, useParams } from 'react-router-dom';

import { CommandPalette } from './components/CommandPalette';
import { api } from './lib/api';
import { useNavShortcuts } from './lib/useShortcuts';

/// Main app shell — sidebar + content outlet. Wraps every
/// authenticated page.
export function App() {
  const [verified, setVerified] = useState(false);
  const [paletteOpen, setPaletteOpen] = useState(false);
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
          // Stash UI display fields so the rest of the app can
          // show them without hitting /auth/me on every render.
          try {
            localStorage.setItem('sentori_user_id', me.user_id);
            localStorage.setItem('sentori_email', me.email);
          } catch {}
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
      <div className="flex h-screen items-center justify-center bg-zinc-950 text-sm text-zinc-500">
        Loading…
      </div>
    );
  }
  return (
    <div className="flex h-screen">
      <Sidebar />
      <main className="flex-1 overflow-y-auto">
        <Outlet />
      </main>
      <CommandPalette
        open={paletteOpen}
        onClose={() => setPaletteOpen(false)}
      />
    </div>
  );
}

function Sidebar() {
  // Sidebar layout mirrors the lens grouping from legacy
  // `web/src/modules/registry.tsx`: workspace-wide pages
  // up top, per-project pages picked when a project is
  // selected.
  const params = useParams<{ id?: string }>();
  const projectScoped = params.id;

  return (
    <aside className="flex w-56 shrink-0 flex-col border-r border-zinc-800 bg-zinc-950 p-4">
      <div className="mb-6">
        <h1 className="text-base font-semibold tracking-tight text-zinc-100">
          Sentori
        </h1>
        <p className="font-mono text-[10px] text-zinc-600">v0.2</p>
      </div>

      <nav className="flex flex-1 flex-col gap-1 text-sm">
        <SectionLabel>Workspace</SectionLabel>
        <NavItem to="/" label="Overview" />
        <NavItem to="/search" label="Search" />
        <NavItem to="/projects" label="Projects" />
        <NavItem to="/members" label="Members" />
        <NavItem to="/alerts" label="Alerts" />
        <NavItem to="/saved-views" label="Saved views" />
        <NotificationsNavItem />
        <NavItem to="/audit" label="Audit" />
        <NavItem to="/settings" label="Settings" />
        <NavItem to="/health" label="Health" />
        <NavItem to="/saas" label="SaaS admin" />

        {projectScoped && (
          <>
            <SectionLabel className="mt-6">Project</SectionLabel>
            <NavItem to={`/projects/${projectScoped}/issues`} label="Issues" />
            <NavItem to={`/projects/${projectScoped}/events`} label="Events" />
            <NavItem to={`/projects/${projectScoped}/traces`} label="Traces" />
            <NavItem
              to={`/projects/${projectScoped}/metrics`}
              label="Metrics"
            />
            <NavItem
              to={`/projects/${projectScoped}/replays`}
              label="Replays"
            />
            <NavItem
              to={`/projects/${projectScoped}/cert`}
              label="Cert monitor"
            />
            <NavItem
              to={`/projects/${projectScoped}/probes`}
              label="Endpoint probes"
            />
            <NavItem to={`/projects/${projectScoped}/tokens`} label="Tokens" />
            <NavItem to={`/projects/${projectScoped}/push`} label="Push" />
            <NavItem
              to={`/projects/${projectScoped}/integrations`}
              label="Integrations"
            />
            <NavItem
              to={`/projects/${projectScoped}/releases`}
              label="Releases"
            />
          </>
        )}
      </nav>

      <UserFooter />
    </aside>
  );
}

function NotificationsNavItem() {
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
            ? 'bg-zinc-800 text-zinc-100'
            : 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200'
        }`
      }
    >
      <span>Inbox</span>
      {unread > 0 && (
        <span className="rounded bg-emerald-600 px-1.5 py-0.5 text-[10px] font-mono text-white">
          {unread}
        </span>
      )}
    </NavLink>
  );
}

function UserFooter() {
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
    } catch {}
    try {
      localStorage.removeItem('sentori_user_id');
      localStorage.removeItem('sentori_email');
    } catch {}
    navigate('/login');
  }

  if (!email) return null;
  return (
    <div className="mt-4 border-t border-zinc-800 pt-3 text-[11px]">
      <div className="flex items-center justify-between gap-1">
        <span
          className="truncate font-mono text-zinc-400"
          title={email}
        >
          {email}
        </span>
        <button
          onClick={signOut}
          title="Sign out"
          className="rounded px-1.5 py-0.5 text-zinc-500 hover:bg-zinc-800 hover:text-zinc-200"
        >
          ⎋
        </button>
      </div>
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
            ? 'bg-zinc-800 text-zinc-100'
            : 'text-zinc-400 hover:bg-zinc-900 hover:text-zinc-200'
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
      className={`mb-1 px-2.5 text-[10px] font-medium uppercase tracking-wider text-zinc-600 ${className}`}
    >
      {children}
    </p>
  );
}
