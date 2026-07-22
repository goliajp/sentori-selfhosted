import { useEffect, useRef, useState } from 'react';

import { useT } from '../i18n';
import { api, MeResponse, MyWorkspaceRow } from '../lib/api';

/// Active-workspace pill + dropdown switcher, shown at the top of
/// the sidebar. Multi-workspace (1:N): a user can belong to several
/// workspaces; picking one repoints the session server-side and
/// reloads so every query re-scopes to the new active workspace.
export function WorkspaceSwitcher({ me }: { me: MeResponse }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const [rows, setRows] = useState<MyWorkspaceRow[] | null>(null);
  const [switching, setSwitching] = useState<string | null>(null);
  const ref = useRef<HTMLDivElement>(null);

  // Close on outside click.
  useEffect(() => {
    if (!open) return;
    function onDoc(e: MouseEvent) {
      if (ref.current && !ref.current.contains(e.target as Node)) {
        setOpen(false);
      }
    }
    document.addEventListener('mousedown', onDoc);
    return () => document.removeEventListener('mousedown', onDoc);
  }, [open]);

  // Lazy-load the membership list the first time the menu opens.
  useEffect(() => {
    if (!open || rows) return;
    api
      .listMyWorkspaces()
      .then(r => setRows(r.workspaces))
      .catch(() => setRows([]));
  }, [open, rows]);

  async function pick(w: MyWorkspaceRow) {
    if (w.active) {
      setOpen(false);
      return;
    }
    setSwitching(w.workspace_id);
    try {
      await api.switchWorkspace(w.workspace_id);
      // Re-scope the whole app: every query keys off the session's
      // active workspace, so a full reload is the clean reset.
      window.location.assign('/');
    } catch {
      setSwitching(null);
    }
  }

  const name = me.workspace_name ?? t('workspace.label');
  return (
    <div ref={ref} className="relative">
      <button
        onClick={() => setOpen(o => !o)}
        className="flex w-full items-center justify-between rounded border border-border bg-surface px-2.5 py-2 text-left hover:border-border-strong"
      >
        <span className="min-w-0">
          <span className="block truncate text-xs font-medium text-fg">
            {name}
          </span>
          <span className="block text-xs text-fg-subtle">{me.role}</span>
        </span>
        <span className="ml-2 shrink-0 text-fg-subtle">▾</span>
      </button>
      {open && (
        <div className="absolute left-0 right-0 z-20 mt-1 max-h-72 overflow-y-auto rounded border border-border bg-surface py-1 shadow-xl">
          {rows === null ? (
            <div className="px-3 py-2 text-xs text-fg-subtle">{t('common.loading')}</div>
          ) : rows.length === 0 ? (
            <div className="px-3 py-2 text-xs text-fg-subtle">
              No workspaces.
            </div>
          ) : (
            rows.map(w => (
              <button
                key={w.workspace_id}
                onClick={() => pick(w)}
                disabled={switching !== null}
                className={`flex w-full items-center justify-between px-3 py-1.5 text-left text-xs hover:bg-raised ${
                  w.active ? 'text-accent' : 'text-fg'
                }`}
              >
                <span className="min-w-0 truncate">{w.name}</span>
                <span className="ml-2 shrink-0 text-xs text-fg-subtle">
                  {switching === w.workspace_id
                    ? '…'
                    : w.active
                      ? '✓'
                      : w.role}
                </span>
              </button>
            ))
          )}
        </div>
      )}
    </div>
  );
}
