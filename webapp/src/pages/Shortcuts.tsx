// Keyboard shortcut cheatsheet. Reachable via `?` or
// /shortcuts URL.

import { PageHeader, Card, Section, CardHeader } from '../components/ui';

const GROUPS: { title: string; items: { kbd: string; label: string }[] }[] =
  [
    {
      title: 'Global navigation',
      items: [
        { kbd: 'g i', label: 'Overview' },
        { kbd: 'g p', label: 'Projects' },
        { kbd: 'g m', label: 'Members' },
        { kbd: 'g a', label: 'Alerts' },
        { kbd: 'g v', label: 'Saved views' },
        { kbd: 'g u', label: 'Audit log' },
        { kbd: 'g n', label: 'Notifications' },
        { kbd: 'g s', label: 'Settings' },
        { kbd: 'g h', label: 'Health' },
        { kbd: 'g o', label: 'SaaS admin' },
      ],
    },
    {
      title: 'Command palette',
      items: [
        { kbd: '⌘K / Ctrl-K', label: 'Open fuzzy nav + backend search' },
        { kbd: '↑ ↓', label: 'Navigate palette items' },
        { kbd: '↵', label: 'Open selected' },
        { kbd: 'esc', label: 'Close' },
      ],
    },
    {
      title: 'Issue list',
      items: [
        { kbd: 'j / k', label: 'Move cursor down / up' },
        { kbd: 'x', label: 'Toggle select cursor row' },
        { kbd: 'e', label: 'Resolve cursor row' },
        { kbd: 'i', label: 'Ignore cursor row' },
      ],
    },
    {
      title: 'Issue detail',
      items: [
        { kbd: 'e', label: 'Resolve' },
        { kbd: 'i', label: 'Ignore' },
        { kbd: 'r', label: 'Reopen' },
        { kbd: 'w', label: 'Toggle Watch' },
      ],
    },
    {
      title: 'Trace / replay detail',
      items: [
        { kbd: 'esc', label: 'Back to list' },
        { kbd: '← →', label: 'Scrub replay frames' },
      ],
    },
    {
      title: 'Misc',
      items: [
        { kbd: '?', label: 'Open this cheatsheet' },
      ],
    },
  ];

export default function Shortcuts() {
  return (
    <div className="space-y-4">
      <PageHeader
        title="Keyboard shortcuts"
        subtitle="Linear-style navigation. Disabled while focus is in an input."
      />
      <div className="grid grid-cols-2 gap-4">
        {GROUPS.map(g => (
          <Card key={g.title}>
            <CardHeader title={g.title} />
            <Section>
              <ul className="space-y-1 text-xs">
                {g.items.map(it => (
                  <li
                    key={it.kbd}
                    className="flex items-center justify-between"
                  >
                    <span className="text-zinc-300">{it.label}</span>
                    <kbd className="rounded bg-zinc-800 px-1.5 py-0.5 font-mono text-[10px] text-zinc-200">
                      {it.kbd}
                    </kbd>
                  </li>
                ))}
              </ul>
            </Section>
          </Card>
        ))}
      </div>
    </div>
  );
}
