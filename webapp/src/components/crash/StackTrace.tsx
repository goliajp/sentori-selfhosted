// The stack, read the way a person reads one.
//
// Two decisions carry this component.
//
// First, `inApp` is the only distinction that earns colour. A crash
// report is mostly frames the reader did not write — React internals,
// the JS engine, the bridge — and the one or two frames that are
// theirs are the whole point. Those get an accent rule and stay open;
// everything else collapses behind a single line telling you how many
// frames it hid.
//
// Second, the frames render innermost-first. The SDK sends them in
// throw order, but the line that threw is what you came to read, so
// it goes at the top where your eye already is.

import { useState } from 'react';

import { useT } from '../../i18n';

import type { CapturedError, StackFrame } from '../../lib/api';

export function StackTrace({ error }: { error: CapturedError }) {
  // A `cause` chain is a list, not a nesting — flatten it so each
  // link renders at the same level with its relationship named.
  const chain: CapturedError[] = [];
  let node: CapturedError | null | undefined = error;
  while (node && chain.length < 12) {
    chain.push(node);
    node = node.cause;
  }

  return (
    <div className="space-y-6">
      {chain.map((link, i) => (
        <ErrorLink key={i} error={link} depth={i} />
      ))}
    </div>
  );
}

function ErrorLink({ error, depth }: { error: CapturedError; depth: number }) {
  const t = useT();
  const frames = [...(error.stack ?? [])].reverse();
  const appFrames = frames.filter(f => f.inApp).length;

  return (
    <section>
      {depth > 0 && (
        <p className="mb-2 font-mono text-xs uppercase tracking-wider text-fg-subtle">
          {t('crash.causedBy')}
        </p>
      )}
      <h3 className="font-mono text-sm text-fg">
        <span className="text-danger">{error.type}</span>
        {error.message && (
          <span className="text-fg-muted">: {error.message}</span>
        )}
      </h3>
      {frames.length > 0 && (
        <div className="mt-3 overflow-hidden rounded border border-border">
          <FrameList frames={frames} appFrames={appFrames} />
        </div>
      )}
    </section>
  );
}

/** Runs of non-app frames collapse into one summary row. */
function FrameList({
  frames,
  appFrames,
}: {
  frames: StackFrame[];
  appFrames: number;
}) {
  // With no in-app frames at all there is nothing to prioritise, so
  // hiding the rest would leave an empty panel.
  const collapseSystem = appFrames > 0;
  const groups: { system: boolean; frames: StackFrame[] }[] = [];
  for (const f of frames) {
    const system = collapseSystem && !f.inApp;
    const last = groups.at(-1);
    if (last && last.system === system) last.frames.push(f);
    else groups.push({ system, frames: [f] });
  }

  return (
    <ul className="divide-y divide-border">
      {groups.map((g, i) =>
        g.system ? (
          <SystemRun key={i} frames={g.frames} />
        ) : (
          g.frames.map((f, j) => <Frame key={`${i}-${j}`} frame={f} />)
        ),
      )}
    </ul>
  );
}

function SystemRun({ frames }: { frames: StackFrame[] }) {
  const t = useT();
  const [open, setOpen] = useState(false);
  return (
    <li className="bg-surface">
      <button
        type="button"
        onClick={() => setOpen(o => !o)}
        aria-expanded={open}
        className="w-full px-3 py-1.5 text-left font-mono text-xs text-fg-subtle transition hover:text-fg-muted focus-visible:outline focus-visible:outline-2 focus-visible:-outline-offset-2 focus-visible:outline-accent"
      >
        {open ? '−' : '+'} {frames.length} {t('crash.framesHidden')}
      </button>
      {open && (
        <ul className="divide-y divide-border border-t border-border">
          {frames.map((f, i) => (
            <Frame key={i} frame={f} />
          ))}
        </ul>
      )}
    </li>
  );
}

function Frame({ frame }: { frame: StackFrame }) {
  const hasSource =
    (frame.preContext?.length ?? 0) > 0 || (frame.postContext?.length ?? 0) > 0;

  return (
    <li
      className={
        frame.inApp
          ? 'border-l-2 border-l-accent bg-surface'
          : 'bg-surface/50'
      }
    >
      <div className="flex items-baseline gap-2 px-3 py-2 font-mono text-xs">
        <span className={frame.inApp ? 'text-fg' : 'text-fg-muted'}>
          {frame.function || '<anonymous>'}
        </span>
        <span className="min-w-0 flex-1 truncate text-right text-fg-subtle">
          {frame.file}
          <span className="text-fg-muted">
            :{frame.line}
            {frame.column ? `:${frame.column}` : ''}
          </span>
        </span>
      </div>
      {hasSource && <SourceContext frame={frame} />}
    </li>
  );
}

/** The lines around the throw, with the throwing line marked. Only
 *  present when the frame was symbolicated against a source map. */
function SourceContext({ frame }: { frame: StackFrame }) {
  const pre = frame.preContext ?? [];
  const post = frame.postContext ?? [];
  const firstLine = frame.line - pre.length;

  return (
    <pre className="overflow-x-auto border-t border-border bg-bg px-3 py-2 font-mono text-xs leading-relaxed">
      {pre.map((l, i) => (
        <Line key={`p${i}`} n={firstLine + i} text={l} />
      ))}
      <Line n={frame.line} text="" highlight />
      {post.map((l, i) => (
        <Line key={`s${i}`} n={frame.line + 1 + i} text={l} />
      ))}
    </pre>
  );
}

function Line({
  n,
  text,
  highlight,
}: {
  n: number;
  text: string;
  highlight?: boolean;
}) {
  return (
    <div className={highlight ? 'bg-danger/10' : undefined}>
      <span className="mr-3 inline-block w-10 select-none text-right text-fg-subtle">
        {n}
      </span>
      <span className={highlight ? 'text-danger' : 'text-fg-muted'}>
        {text}
      </span>
    </div>
  );
}
