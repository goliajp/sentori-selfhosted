// Linear-style command palette. Cmd-K (Mac) / Ctrl-K (others)
// opens. Type to filter. Esc / outside-click closes.
//
// Sources (no backend search call): static nav items + project
// list cached in localStorage by the Overview load.

import { useEffect, useMemo, useRef, useState } from 'react';
import { useNavigate } from 'react-router-dom';

import { api } from '../lib/api';

interface PaletteItem {
  id: string;
  label: string;
  hint?: string;
  route: string;
}

const WORKSPACE_ROUTES: PaletteItem[] = [
  { id: 'wo', label: 'Overview', hint: 'g i', route: '/' },
  { id: 'wp', label: 'Projects', hint: 'g p', route: '/projects' },
  { id: 'wm', label: 'Members', hint: 'g m', route: '/members' },
  { id: 'wa', label: 'Alerts', hint: 'g a', route: '/alerts' },
  {
    id: 'wv',
    label: 'Saved views',
    hint: 'g v',
    route: '/saved-views',
  },
  { id: 'wu', label: 'Audit log', hint: 'g u', route: '/audit' },
  { id: 'ws', label: 'Settings', hint: 'g s', route: '/settings' },
  { id: 'wh', label: 'Health', hint: 'g h', route: '/health' },
  { id: 'wsa', label: 'SaaS admin', hint: 'g o', route: '/saas' },
];

interface Props {
  open: boolean;
  onClose: () => void;
}

export function CommandPalette({ open, onClose }: Props) {
  const navigate = useNavigate();
  const inputRef = useRef<HTMLInputElement>(null);
  const [query, setQuery] = useState('');
  const [selected, setSelected] = useState(0);
  const [projects, setProjects] = useState<PaletteItem[]>([]);
  const [searchHits, setSearchHits] = useState<PaletteItem[]>([]);

  // Reset + autofocus on open + load projects.
  useEffect(() => {
    if (!open) return;
    setQuery('');
    setSelected(0);
    setTimeout(() => inputRef.current?.focus(), 10);
    api
      .listProjects()
      .then(rows => {
        const items = rows.flatMap((p): PaletteItem[] => [
          {
            id: `pi-${p.id}`,
            label: `Issues · ${p.name}`,
            hint: p.slug,
            route: `/projects/${p.id}/issues`,
          },
          {
            id: `pe-${p.id}`,
            label: `Events · ${p.name}`,
            hint: p.slug,
            route: `/projects/${p.id}/events`,
          },
          {
            id: `pt-${p.id}`,
            label: `Traces · ${p.name}`,
            hint: p.slug,
            route: `/projects/${p.id}/traces`,
          },
          {
            id: `pm-${p.id}`,
            label: `Metrics · ${p.name}`,
            hint: p.slug,
            route: `/projects/${p.id}/metrics`,
          },
          {
            id: `pr-${p.id}`,
            label: `Replays · ${p.name}`,
            hint: p.slug,
            route: `/projects/${p.id}/replays`,
          },
        ]);
        setProjects(items);
      })
      .catch(() => {});
  }, [open]);

  const items = useMemo(() => {
    const all = [...WORKSPACE_ROUTES, ...projects, ...searchHits];
    const q = query.trim().toLowerCase();
    if (!q) return all.slice(0, 50);
    return all
      .filter(
        i =>
          i.label.toLowerCase().includes(q) ||
          (i.hint?.toLowerCase().includes(q) ?? false),
      )
      .slice(0, 50);
  }, [query, projects, searchHits]);

  // Backend search: when query > 2 chars, fire searchProject
  // against the first project in the workspace (good enough for
  // single-project self-hosted; future improvement: per-project
  // scope selector).
  useEffect(() => {
    const q = query.trim();
    if (q.length < 3) {
      setSearchHits([]);
      return;
    }
    const timer = setTimeout(async () => {
      try {
        const ps = await api.listProjects();
        if (!ps[0]) return;
        const projectId = ps[0].id;
        const r = await api.searchProject(projectId, q, 8);
        const hits: PaletteItem[] = [
          ...r.issues.map(i => ({
            id: `si-${i.id}`,
            label: `[${i.status}] ${i.error_type}`,
            hint: i.message_sample.slice(0, 30),
            route: `/projects/${projectId}/issues/${i.id}`,
          })),
          ...r.events.map(e => ({
            id: `se-${e.id}`,
            label: `event ${e.kind} · ${e.release}`,
            hint: e.environment,
            route: `/projects/${projectId}/issues/${e.issue_id}`,
          })),
        ];
        setSearchHits(hits);
      } catch {
        setSearchHits([]);
      }
    }, 200);
    return () => clearTimeout(timer);
  }, [query]);

  useEffect(() => {
    setSelected(0);
  }, [query]);

  if (!open) return null;

  function fire(item: PaletteItem) {
    onClose();
    navigate(item.route);
  }

  function onKey(e: React.KeyboardEvent) {
    if (e.key === 'Escape') {
      onClose();
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      setSelected(s => Math.min(items.length - 1, s + 1));
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      setSelected(s => Math.max(0, s - 1));
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (items[selected]) fire(items[selected]);
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center bg-black/60 p-4 pt-24"
      onClick={onClose}
    >
      <div
        className="w-full max-w-xl rounded-lg border border-zinc-700 bg-zinc-900 shadow-xl"
        onClick={e => e.stopPropagation()}
      >
        <input
          ref={inputRef}
          value={query}
          onChange={e => setQuery(e.target.value)}
          onKeyDown={onKey}
          placeholder="Jump to…  (try project name, or 'issues')"
          className="w-full border-b border-zinc-800 bg-transparent px-4 py-3 text-sm text-zinc-100 placeholder:text-zinc-600 focus:outline-none"
        />
        <ul className="max-h-80 overflow-y-auto py-1">
          {items.length === 0 ? (
            <li className="px-4 py-3 text-xs text-zinc-500">
              No matches
            </li>
          ) : (
            items.map((item, i) => (
              <li
                key={item.id}
                onMouseEnter={() => setSelected(i)}
                onClick={() => fire(item)}
                className={`flex cursor-pointer items-center justify-between px-4 py-2 text-sm ${
                  i === selected
                    ? 'bg-zinc-800 text-zinc-100'
                    : 'text-zinc-300'
                }`}
              >
                <span>{item.label}</span>
                {item.hint && (
                  <span className="font-mono text-[10px] text-zinc-500">
                    {item.hint}
                  </span>
                )}
              </li>
            ))
          )}
        </ul>
        <div className="border-t border-zinc-800 px-4 py-2 text-[10px] text-zinc-600">
          ↑↓ navigate · ↵ open · esc close
        </div>
      </div>
    </div>
  );
}
