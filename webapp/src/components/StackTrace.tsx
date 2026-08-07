// The stack, read as evidence — not a wall of text.
//
// In-app frames carry their source window (the server resolves it
// from the sourcemap's embedded sourcesContent; no repository
// access anywhere) and open by default: the reader should see the
// failing line without a click. Library frames collapse into a
// single count row — they are context, not suspects.
//
// When the classifier finds NO in-app frame (dev bundles before the
// SDK's Metro symbolication, exotic runtimes), folding would leave
// the reader a single "13 library frames" row and nothing else — so
// in that case every frame is shown flat. An empty-looking stack is
// a UI bug, not a data property.

import { ChevronRight } from 'lucide-react';
import { useState, type ReactNode } from 'react';

import { useT } from '../i18n';

export type StackFrame = {
  file?: string;
  function?: string;
  line?: number;
  column?: number;
  inApp?: boolean;
  symbolicated?: boolean;
  preContext?: string[];
  contextLine?: string;
  postContext?: string[];
};

const MAX_FRAMES = 40;

/** Strip the directory prefix every frame shares — dev builds carry
 *  the developer's absolute machine paths (`/Users/x/workspace/…`),
 *  which are noise AND leak the local username onto a shared board.
 *  The common prefix is computed per stack, so release-build
 *  repo-relative paths pass through untouched. The full path stays
 *  in the hover. */
function commonPrefix(paths: string[]): string {
  const splitted = paths
    .filter((p) => p.startsWith('/'))
    .map((p) => p.split('/').slice(0, -1));
  if (splitted.length < 2) return '';
  let prefix = splitted[0]!;
  for (const segs of splitted.slice(1)) {
    let i = 0;
    while (i < prefix.length && i < segs.length && prefix[i] === segs[i]) i++;
    prefix = prefix.slice(0, i);
  }
  // Only strip a real machine prefix (at least /Users/x or /home/x),
  // and keep the repo dir itself readable.
  return prefix.length >= 3 ? prefix.join('/') + '/' : '';
}

/** `minified` means the reader is looking at bundle coordinates.
 *  A dev-client stack symbolicated against Metro has real function
 *  names and file paths but (pre-5.3 SDKs) no `symbolicated` flag —
 *  flagging those frames MINIFIED is wrong. The badge shows only
 *  when the location actually looks like a bundle. */
function looksMinified(f: StackFrame): boolean {
  if (f.symbolicated === true) return false;
  const file = f.file ?? '';
  return /^https?:\/\//.test(file) || /\.(bundle|jsbundle)/.test(file);
}

export function StackTrace({ frames }: { frames: StackFrame[] }) {
  const t = useT();
  const shown = frames.slice(0, MAX_FRAMES);
  const anyInApp = shown.some((f) => f.inApp === true);
  const strip = commonPrefix(shown.map((f) => f.file ?? ''));

  // Group runs of library frames so they collapse to one row each —
  // but only when there are in-app frames to anchor the reading.
  const groups: { frames: { f: StackFrame; i: number }[]; inApp: boolean }[] = [];
  shown.forEach((f, i) => {
    const inApp = !anyInApp || f.inApp === true;
    const last = groups.at(-1);
    if (last && last.inApp === inApp) last.frames.push({ f, i });
    else groups.push({ frames: [{ f, i }], inApp });
  });

  return (
    <div className="overflow-hidden">
      {groups.map((g, gi) =>
        g.inApp ? (
          g.frames.map(({ f, i }) => (
            <AppFrame key={i} frame={f} defaultOpen={i < 3} strip={strip} />
          ))
        ) : (
          <LibraryRun key={`lib-${gi}`} frames={g.frames} strip={strip} />
        ),
      )}
      {frames.length > MAX_FRAMES && (
        <div className="border-t border-border px-3.5 py-1.5 font-mono text-xs text-fg-subtle">
          {t('stack.truncated', { n: String(frames.length - MAX_FRAMES) })}
        </div>
      )}
    </div>
  );
}

/** One in-app frame: header row + (when the server resolved it) the
 *  source window. */
function AppFrame({
  frame,
  defaultOpen,
  strip,
}: {
  frame: StackFrame;
  defaultOpen: boolean;
  strip: string;
}) {
  const t = useT();
  const hasContext = typeof frame.contextLine === 'string';
  const [open, setOpen] = useState(defaultOpen && hasContext);
  const shownFile = frame.file?.startsWith(strip)
    ? frame.file.slice(strip.length)
    : frame.file;

  return (
    <div className="border-b border-border last:border-b-0">
      <button
        type="button"
        disabled={!hasContext}
        onClick={() => setOpen((o) => !o)}
        aria-expanded={hasContext ? open : undefined}
        className={`flex w-full items-baseline gap-2 px-3.5 py-2 text-left font-mono text-sm ${
          hasContext ? 'cursor-pointer hover:bg-raised' : 'cursor-default'
        }`}
      >
        <ChevronRight
          aria-hidden
          className={`h-3.5 w-3.5 shrink-0 transition-transform ${
            hasContext ? 'text-fg-subtle' : 'opacity-0'
          } ${open ? 'rotate-90' : ''}`}
        />
        {/* fixed-width function column: locations start on one
            vertical line, so the eye can scan either column — free
            widths interleaved names and paths into an unreadable
            zigzag */}
        <span
          className="w-64 shrink-0 truncate font-medium text-fg"
          title={frame.function}
        >
          {frame.function ?? '?'}
        </span>
        <span
          className="min-w-0 flex-1 truncate text-fg-subtle"
          title={frame.file}
        >
          {shownFile ?? '?'}
          {frame.line !== undefined ? `:${frame.line}` : ''}
          {frame.column !== undefined ? `:${frame.column}` : ''}
        </span>
        {looksMinified(frame) && (
          <span className="shrink-0 rounded border border-border-strong px-1 text-xs uppercase tracking-wide text-fg-subtle">
            {t('stack.minified')}
          </span>
        )}
      </button>
      {open && hasContext && <SourceWindow frame={frame} />}
    </div>
  );
}

// ── syntax tint ────────────────────────────────────────────
//
// A deliberately tiny per-line tokenizer, not a grammar: strings,
// comments, numbers and a shared keyword set covering the languages
// that actually reach this window (TS/JS from the sourcemap, Swift
// and Kotlin from the srcbundle). Zero dependencies — a real
// highlighter ships more grammar than this whole dashboard. Hues
// come from the five-kind palette, so both themes are already
// re-inked.

const KEYWORDS = new Set(
  (
    'const let var function return if else for while do try catch finally ' +
    'throw new class extends import export from default await async switch ' +
    'case break continue typeof instanceof in of delete void yield ' +
    'null undefined true false this super static readonly interface type ' +
    'enum implements declare public private protected abstract ' +
    // Swift / Kotlin
    'func val fun guard defer struct protocol extension where when object ' +
    'companion override open data sealed internal lazy weak init self nil ' +
    'package'
  ).split(' '),
);

const TOKEN_RE =
  /(\/\/.*$)|("(?:[^"\\]|\\.)*"?|'(?:[^'\\]|\\.)*'?|`(?:[^`\\]|\\.)*`?)|(\b\d[\d_]*(?:\.\d+)?\b)|([A-Za-z_$][\w$]*)/g;

const TOKEN_INK: Record<string, string> = {
  comment: 'var(--sn-fg-subtle)',
  keyword: 'var(--s-kind-assert)',
  number: 'var(--s-kind-warn)',
  string: 'var(--s-kind-probe)',
};

function highlightLine(text: string): ReactNode {
  // A line living inside a block comment (leading * or /*) reads as
  // one — the per-line scan can't track /* … */ across rows.
  if (/^\s*(\*|\/\*)/.test(text)) {
    return <span style={{ color: TOKEN_INK.comment }}>{text}</span>;
  }
  const out: ReactNode[] = [];
  let last = 0;
  let k = 0;
  TOKEN_RE.lastIndex = 0;
  for (let m = TOKEN_RE.exec(text); m; m = TOKEN_RE.exec(text)) {
    if (m.index > last) out.push(text.slice(last, m.index));
    const [tok, comment, str, num, word] = m;
    const ink = comment
      ? TOKEN_INK.comment
      : str
        ? TOKEN_INK.string
        : num
          ? TOKEN_INK.number
          : word && KEYWORDS.has(word)
            ? TOKEN_INK.keyword
            : null;
    out.push(
      ink ? (
        <span key={k++} style={{ color: ink }}>
          {tok}
        </span>
      ) : (
        tok
      ),
    );
    last = m.index + tok.length;
  }
  if (last < text.length) out.push(text.slice(last));
  return out;
}

/** The reading window around the failing line, numbered from the
 *  resolved position. The hit line carries the tint + red gutter. */
function SourceWindow({ frame }: { frame: StackFrame }) {
  const pre = frame.preContext ?? [];
  const post = frame.postContext ?? [];
  const hitLine = frame.line ?? 0;
  const start = hitLine - pre.length;
  const rows: { n: number; text: string; hit: boolean }[] = [
    ...pre.map((text, i) => ({ n: start + i, text, hit: false })),
    { n: hitLine, text: frame.contextLine ?? '', hit: true },
    ...post.map((text, i) => ({ n: hitLine + 1 + i, text, hit: false })),
  ];

  return (
    <div className="overflow-x-auto border-t border-border bg-bg">
      {/* code sits a step below the UI floor on purpose — density is
          the point of a reading window */}
      <table className="w-full border-collapse font-mono text-xs leading-5">
        <tbody>
          {rows.map((r) => (
            <tr
              key={r.n}
              style={
                r.hit
                  ? { backgroundColor: 'color-mix(in srgb, var(--s-kind-error) 9%, transparent)' }
                  : undefined
              }
            >
              <td
                className={`w-px select-none border-r py-0 pl-3.5 pr-2 text-right align-top ${
                  r.hit
                    ? 'border-kind-error text-kind-error'
                    : 'border-border text-fg-subtle/70'
                }`}
              >
                {r.n}
              </td>
              <td className="whitespace-pre py-0 pl-3.5 pr-4 text-fg">
                {r.text ? highlightLine(r.text) : ' '}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}

/** A run of consecutive library frames, folded to one row. */
function LibraryRun({
  frames,
  strip,
}: {
  frames: { f: StackFrame; i: number }[];
  strip: string;
}) {
  const t = useT();
  const [open, setOpen] = useState(false);

  return (
    <div className="border-b border-border last:border-b-0">
      <button
        type="button"
        onClick={() => setOpen((o) => !o)}
        aria-expanded={open}
        className="flex w-full items-baseline gap-2 px-3.5 py-1.5 text-left font-mono text-xs text-fg-subtle hover:bg-raised hover:text-fg-muted"
      >
        <ChevronRight
          aria-hidden
          className={`h-3.5 w-3.5 shrink-0 transition-transform ${open ? 'rotate-90' : ''}`}
        />
        {open
          ? t('stack.libraryFramesOpen')
          : t('stack.libraryFrames', { n: String(frames.length) })}
      </button>
      {open &&
        frames.map(({ f, i }) => (
          <div
            key={i}
            className="flex items-baseline gap-2 py-0.5 pl-8 pr-3.5 font-mono text-xs text-fg-subtle"
          >
            {/* same two-column discipline as the app frames */}
            <span className="w-60 shrink-0 truncate" title={f.function}>
              {f.function ?? '?'}
            </span>
            <span className="min-w-0 flex-1 truncate" title={f.file}>
              {(f.file?.startsWith(strip) ? f.file.slice(strip.length) : f.file) ?? '?'}
              {f.line !== undefined ? `:${f.line}` : ''}
            </span>
          </div>
        ))}
    </div>
  );
}
