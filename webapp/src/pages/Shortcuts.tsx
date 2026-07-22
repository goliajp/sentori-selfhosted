// Keyboard shortcut cheatsheet. Reachable via `?` or
// /shortcuts URL.

import { useT } from '../i18n';
import type { MessageKey } from '../i18n/en';
import { PageHeader, Card, CardBody, CardHeader } from '../components/ui';

/**
 * The cheatsheet, carrying message keys.
 *
 * The navigation half lists the same destinations as the sidebar, so
 * it shares their `nav.*` entries — a cheatsheet that said "Overview"
 * beside a sidebar reading 概览 would be teaching the wrong word.
 */
const GROUPS: { title: MessageKey; items: { kbd: string; label: MessageKey }[] }[] = [
  {
    title: 'shortcuts.navigation',
    items: [
      { kbd: 'g i', label: 'nav.overview' },
      { kbd: 'g p', label: 'nav.projects' },
      { kbd: 'g m', label: 'nav.members' },
      { kbd: 'g a', label: 'nav.alerts' },
      { kbd: 'g v', label: 'nav.savedViews' },
      { kbd: 'g u', label: 'nav.audit' },
      { kbd: 'g n', label: 'notifications.title' },
      { kbd: 'g s', label: 'nav.settings' },
      { kbd: 'g h', label: 'nav.health' },
      { kbd: 'g o', label: 'nav.saasAdmin' },
    ],
  },
  {
    title: 'shortcuts.palette',
    items: [
      { kbd: '⌘K / Ctrl-K', label: 'shortcuts.openPalette' },
      { kbd: '↑ ↓', label: 'shortcuts.movePalette' },
      { kbd: '↵', label: 'shortcuts.openSelected' },
      { kbd: 'esc', label: 'shortcuts.close' },
    ],
  },
  {
    title: 'shortcuts.issueList',
    items: [
      { kbd: 'j / k', label: 'shortcuts.moveCursor' },
      { kbd: 'x', label: 'shortcuts.toggleSelect' },
      { kbd: 'e', label: 'shortcuts.resolveRow' },
      { kbd: 'i', label: 'shortcuts.ignoreRow' },
    ],
  },
  {
    title: 'shortcuts.issueDetail',
    items: [
      { kbd: 'e', label: 'issues.resolve' },
      { kbd: 'i', label: 'issues.ignore' },
      { kbd: 'r', label: 'issues.reopen' },
      { kbd: 'w', label: 'shortcuts.toggleWatch' },
    ],
  },
  {
    title: 'shortcuts.playback',
    items: [
      { kbd: 'esc', label: 'shortcuts.backToList' },
      { kbd: '← →', label: 'shortcuts.scrub' },
    ],
  },
  {
    title: 'shortcuts.misc',
    items: [{ kbd: '?', label: 'shortcuts.openSheet' }],
  },
];

export default function Shortcuts() {
  const t = useT();
  return (
    <div className="space-y-4">
      <PageHeader
        title={t('shortcuts.title')}
        subtitle={t('shortcuts.subtitle')}
      />
      <div className="grid grid-cols-2 gap-4">
        {GROUPS.map(g => (
          <Card key={t(g.title)}>
            <CardHeader title={t(g.title)} />
            <CardBody>
              <ul className="space-y-1 text-xs">
                {g.items.map(it => (
                  <li
                    key={it.kbd}
                    className="flex items-center justify-between"
                  >
                    <span className="text-fg-muted">{t(it.label)}</span>
                    <kbd className="rounded bg-raised px-1.5 py-0.5 font-mono text-xs text-fg">
                      {it.kbd}
                    </kbd>
                  </li>
                ))}
              </ul>
            </CardBody>
          </Card>
        ))}
      </div>
    </div>
  );
}
