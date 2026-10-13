import { Action } from '../../shared/design/controls';
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
import { useId, useState } from 'react';
import { Link } from 'react-router-dom';

import { useT } from '../i18n';
import { languageForPath } from '../lib/highlight';
import { Code } from '../../shared/documents/Code';
import { useSharedT } from '../i18n/shared';

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
  addr?: number;
  imageBase?: number;
  imageUuid?: string;
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

export function StackTrace({ frames, projectId, release, releaseId, selected, onSelect }: { frames: StackFrame[]; projectId?: string; release?: string; releaseId?: string; selected?: number; onSelect?: (index:number)=>void }) {
  const t = useT();
  const shown = frames.slice(0, MAX_FRAMES).map(frame=>({
    ...frame,
    file: typeof frame.file==='string'?frame.file:undefined,
    function: typeof frame.function==='string'?frame.function:undefined,
    line: typeof frame.line==='number'&&Number.isInteger(frame.line)&&frame.line>0?frame.line:undefined,
    column: typeof frame.column==='number'&&Number.isInteger(frame.column)&&frame.column>0?frame.column:undefined,
    contextLine: typeof frame.contextLine==='string'?frame.contextLine:undefined,
    preContext: Array.isArray(frame.preContext)?frame.preContext.filter(line=>typeof line==='string'):[],
    postContext: Array.isArray(frame.postContext)?frame.postContext.filter(line=>typeof line==='string'):[],
  }));
  const anyInApp = shown.some((f) => f.inApp === true);
  const strip = commonPrefix(shown.map((f) => f.file ?? ''));
  const sourceFrames = shown.filter(f => typeof f.contextLine === 'string').length;

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
    <div className="stack-trace">
      <div className="stack-source-summary">
        <span>{t('stack.sourceCount', {n: String(sourceFrames), total: String(shown.length)})}</span>
        {projectId && release && <Link className="text-link" to={`/releases?project=${encodeURIComponent(projectId)}&${releaseId?'detail='+encodeURIComponent(releaseId):'q='+encodeURIComponent(release)}`}>{t('stack.debugFiles')}</Link>}
        {sourceFrames === 0 && <p>{shown.some(f => f.imageUuid) ? t('stack.nativeSourceMissing') : t('stack.sourceMissing')}</p>}
      </div>
      {groups.map((g, gi) =>
        g.inApp ? (
          g.frames.map(({ f, i }) => (
            <AppFrame key={i} frame={f} index={i} defaultOpen={i < 3} strip={strip} selected={selected===i} onSelect={onSelect} />
          ))
        ) : (
          <LibraryRun key={`lib-${gi}`} frames={g.frames} strip={strip} selected={selected} onSelect={onSelect} />
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
  index,
  selected,
  onSelect,
}: {
  frame: StackFrame;
  defaultOpen: boolean;
  strip: string;
  index: number;
  selected?: boolean;
  onSelect?: (index:number)=>void;
}) {
  const t = useT();
  const detailsId = useId();
  const hasContext = typeof frame.contextLine === 'string';
  const [open, setOpen] = useState(defaultOpen && hasContext);
  const shownFile = frame.file?.startsWith(strip)
    ? frame.file.slice(strip.length)
    : frame.file;

  return (
    <div className="stack-frame" data-open={open} data-selected={selected||undefined}>
      <Action
        type="button"
        onClick={() => { setOpen((o) => !o); onSelect?.(index); }}
        aria-expanded={open}
        aria-controls={detailsId}
        className="stack-frame-heading"
      >
        <span className="stack-frame-number">{index + 1}</span>
        <span className="stack-frame-description">
          <span className="stack-frame-function" title={frame.function}>{frame.function ?? '?'}</span>
          <span className="stack-frame-location" title={frame.file}>
            {shownFile ?? '?'}{frame.line !== undefined ? `:${frame.line}` : ''}{frame.line !== undefined && frame.column !== undefined ? `:${frame.column}` : ''}
          </span>
        </span>
        <span className="stack-frame-state">{hasContext ? t('stack.sourceReady') : looksMinified(frame) ? t('stack.minified') : frame.line !== undefined ? t('stack.sourceUnavailable') : t('stack.locationUnresolved')}</span>
        <ChevronRight
          aria-hidden
          className={`stack-frame-chevron ${open ? 'rotate-90' : ''}`}
        />
      </Action>
      {open && <div id={detailsId} className="stack-frame-details">
        {hasContext ? <SourceWindow frame={frame} /> : <div className="stack-frame-unavailable">
          <p>{frame.line !== undefined ? t('stack.sourceMissing') : frame.imageUuid ? t('stack.nativeLocationMissing') : t('stack.locationMissing')}</p>
          {frame.imageUuid && <dl>
            <div><dt>{t('stack.binaryUuid')}</dt><dd>{frame.imageUuid}</dd></div>
            {typeof frame.addr === 'number' && Number.isSafeInteger(frame.addr) && <div><dt>{t('stack.address')}</dt><dd>0x{frame.addr.toString(16)}</dd></div>}
          </dl>}
        </div>}
      </div>}
    </div>
  );
}

// ── syntax tint ────────────────────────────────────────────
//
// A grammar, not a scan. What stood here was a per-line tokenizer
// whose own comment named what it could not do — "the per-line scan
// can't track /* … */ across rows" — so a block comment in the middle
// of a source window came out as code. The window is highlighted
// whole and then cut into rows, which is the only way that case works.
//
// The language comes from the frame's path: an unknown extension gets
// no colour rather than a guess, because mis-lighting a source window
// invites a reader to trust the wrong token.

/** The reading window around the failing line, numbered from the
 *  resolved position. The hit line carries the tint + red gutter. */
function SourceWindow({ frame }: { frame: StackFrame }) {
  const t=useSharedT();
  const pre=frame.preContext??[],post=frame.postContext??[];
  const start=frame.line===undefined?1:Math.max(1,frame.line-pre.length);
  return <Code t={t} code={[...pre,frame.contextLine??'',...post].join('\n')}
    fileName={frame.file} language={languageForPath(frame.file)??'text'}
    startLine={start} lineNumbers={frame.line!==undefined}
    highlightedLines={{[start+pre.length]:'error'}}/>;
}

/** A run of consecutive library frames, folded to one row. */
function LibraryRun({
  frames,
  strip,
  selected,
  onSelect,
}: {
  selected?: number;
  onSelect?: (index:number)=>void;
  frames: { f: StackFrame; i: number }[];
  strip: string;
}) {
  const t = useT();
  const [open, setOpen] = useState(false);

  return (
    <div className="border-b border-border last:border-b-0">
      <Action
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
      </Action>
      {open &&
        frames.map(({ f, i }) => <AppFrame key={i} frame={f} index={i} defaultOpen={false} strip={strip} selected={selected===i} onSelect={onSelect} />)}
    </div>
  );
}
