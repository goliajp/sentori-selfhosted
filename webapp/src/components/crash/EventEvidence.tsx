// Everything Sentori collected about one crash, on one screen.
//
// Order is by what a person reaches for: the stack answers "where",
// the timeline answers "after what", the context answers "on which
// device, which build, which user", and the attachments are the
// artefacts you open when the first three were not enough.
//
// Sections that have no data do not render an empty shell — an SDK
// that collected no breadcrumbs should not leave a "Breadcrumbs (0)"
// box implying something is broken.

import { useState } from 'react';

import { useT } from '../../i18n';

import { api, type EventAttachment, type EventDetail } from '../../lib/api';
import { ReplayPlayer } from './ReplayPlayer';
import { BreadcrumbTimeline } from './BreadcrumbTimeline';
import { StackTrace } from './StackTrace';

export function EventEvidence({
  event,
  projectId,
}: {
  event: EventDetail;
  projectId: string;
}) {
  const t = useT();
  const p = event.payload ?? {};
  const breadcrumbs = p.breadcrumbs ?? [];
  const replay = event.attachments.find(a => a.kind === 'replay');
  // Shared between the recording and the log so scrubbing one moves
  // the other.
  const [playheadTs, setPlayheadTs] = useState<number | undefined>();

  return (
    <div className="space-y-8">
      {p.error ? (
        <Panel title={t('crash.stack')}>
          <StackTrace error={p.error} />
        </Panel>
      ) : (
        p.message && (
          <Panel title={t('crash.message')}>
            <p className="font-mono text-sm text-fg">{p.message}</p>
          </Panel>
        )
      )}

      {(replay || breadcrumbs.length > 0) && (
        <Panel
          title={t('crash.timeline')}
          note={
            replay
              ? t('crash.sharedPlayhead')
              : `${breadcrumbs.length} ${t(breadcrumbs.length === 1 ? 'crash.step' : 'crash.steps')}`
          }
        >
          <div className="grid gap-6 lg:grid-cols-[minmax(0,380px)_minmax(0,1fr)]">
            {replay && (
              <ReplayPlayer
                projectId={projectId}
                attachmentRef={replay.ref}
                onSeek={setPlayheadTs}
              />
            )}
            {breadcrumbs.length > 0 && (
              <BreadcrumbTimeline
                breadcrumbs={breadcrumbs}
                crashedAt={event.timestamp}
                playheadTs={playheadTs}
              />
            )}
          </div>
        </Panel>
      )}

      <Panel title={t('crash.context')}>
        <ContextGrid event={event} />
      </Panel>

      {event.attachments.filter(a => a.kind !== 'replay').length > 0 && (
        <Panel title={t('crash.artefacts')}>
          <Attachments
            items={event.attachments.filter(a => a.kind !== 'replay')}
            projectId={projectId}
          />
        </Panel>
      )}
    </div>
  );
}

function Panel({
  title,
  note,
  children,
}: {
  title: string;
  note?: string;
  children: React.ReactNode;
}) {
  return (
    <section>
      <div className="mb-3 flex items-baseline gap-3">
        <h2 className="text-xs font-medium uppercase tracking-wider text-fg-subtle">
          {title}
        </h2>
        {note && <span className="text-xs text-fg-subtle">{note}</span>}
      </div>
      {children}
    </section>
  );
}

function ContextGrid({ event }: { event: EventDetail }) {
  const t = useT();
  const p = event.payload ?? {};
  const groups: { label: string; rows: [string, string][] }[] = [];

  const push = (label: string, rows: ([string, string | undefined])[]) => {
    const kept = rows.filter((r): r is [string, string] => Boolean(r[1]));
    if (kept.length) groups.push({ label, rows: kept });
  };

  push(t('crash.release'), [
    ['version', event.release],
    ['environment', event.environment],
    ['platform', event.platform],
    ['app build', p.app?.build],
    ['framework', p.app?.framework?.name && `${p.app.framework.name} ${p.app.framework.version ?? ''}`.trim()],
  ]);
  push(t('crash.device'), [
    ['os', [p.device?.os, p.device?.osVersion].filter(Boolean).join(' ')],
    ['model', p.device?.model],
    ['locale', p.device?.locale],
    ['network', p.device?.networkType],
  ]);
  push(t('crash.user'), [
    ['id', p.user?.id],
    ['name', p.user?.name],
    ['anonymous', p.user?.anonymous ? 'yes' : undefined],
    // linkHashes are salted digests by construction — showing that a
    // link exists is useful, showing the digest is noise.
    [
      'linked identities',
      p.user?.linkHashes && Object.keys(p.user.linkHashes).length
        ? Object.keys(p.user.linkHashes).join(', ')
        : undefined,
    ],
  ]);
  push(
    t('crash.tags'),
    Object.entries(p.tags ?? {}).map(([k, v]) => [k, v] as [string, string]),
  );

  if (!groups.length) {
    return (
      <p className="text-sm text-fg-subtle">
        {t('crash.noContext')}
      </p>
    );
  }

  return (
    <div className="grid gap-x-8 gap-y-6 sm:grid-cols-2 lg:grid-cols-4">
      {groups.map(g => (
        <div key={g.label}>
          <p className="mb-2 text-xs uppercase tracking-wide text-fg-subtle">
            {g.label}
          </p>
          <dl className="space-y-1">
            {g.rows.map(([k, v]) => (
              <div key={k} className="flex gap-2 text-xs">
                <dt className="w-24 shrink-0 truncate text-fg-subtle">{k}</dt>
                <dd className="min-w-0 flex-1 truncate font-mono text-fg-muted">
                  {v}
                </dd>
              </div>
            ))}
          </dl>
        </div>
      ))}
    </div>
  );
}

function Attachments({
  items,
  projectId,
}: {
  items: EventAttachment[];
  projectId: string;
}) {
  return (
    <ul className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
      {items.map(a => {
        const href = api.attachmentUrl(projectId, a.ref);
        const isImage = a.media_type.startsWith('image/');
        return (
          <li
            key={a.ref}
            className="overflow-hidden rounded border border-border bg-surface"
          >
            {isImage && (
              <a href={href} target="_blank" rel="noreferrer">
                <img
                  src={href}
                  alt={a.kind}
                  className="max-h-56 w-full object-cover"
                />
              </a>
            )}
            <div className="flex items-baseline justify-between gap-2 px-3 py-2">
              <span className="font-mono text-xs text-fg">{a.kind}</span>
              <a
                href={href}
                target="_blank"
                rel="noreferrer"
                className="text-xs text-accent hover:underline"
              >
                {formatBytes(a.size_bytes)}
              </a>
            </div>
          </li>
        );
      })}
    </ul>
  );
}

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}
