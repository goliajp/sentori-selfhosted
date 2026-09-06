// Syntax colour, from a real grammar.
//
// Two places in this console show code: the source window under a
// stack frame, and the server snippets on Push ▸ Integrate. Both used
// to be lit by a hand-written per-line scanner whose own comment
// admitted what it could not do — "the per-line scan can't track
// /* … */ across rows". A block comment in the middle of a source
// window came out as code.
//
// highlight.js, with the eight grammars this console can actually
// meet and nothing else. Synchronous, so no loading state; class
// names rather than inline colour, so the palette below is ours and
// the light/dark/system trio is the one the rest of the app already
// solves.
//
// ## Lossless or nothing
//
// A highlighter that drops a character is corrupting code someone is
// about to paste into a terminal. `devtools/check-highlight.mjs`
// strips the tags back off and compares with the input, for every
// snippet this console ships and for the line splitter below.

import hljs from 'highlight.js/lib/core';
import bash from 'highlight.js/lib/languages/bash';
import cpp from 'highlight.js/lib/languages/cpp';
import csharp from 'highlight.js/lib/languages/csharp';
import go from 'highlight.js/lib/languages/go';
import java from 'highlight.js/lib/languages/java';
import javascript from 'highlight.js/lib/languages/javascript';
import json from 'highlight.js/lib/languages/json';
import kotlin from 'highlight.js/lib/languages/kotlin';
import python from 'highlight.js/lib/languages/python';
import rust from 'highlight.js/lib/languages/rust';
import swift from 'highlight.js/lib/languages/swift';
import typescript from 'highlight.js/lib/languages/typescript';

for (const [name, lang] of [
  ['bash', bash],
  ['cpp', cpp],
  ['csharp', csharp],
  ['go', go],
  ['java', java],
  ['javascript', javascript],
  ['json', json],
  ['kotlin', kotlin],
  ['python', python],
  ['rust', rust],
  ['swift', swift],
  ['typescript', typescript],
] as const) {
  hljs.registerLanguage(name, lang);
}

/** Which grammar a stack frame's file is written in.
 *
 *  A frame carries a path, not a language. The extension is the only
 *  thing that knows, and an unknown one gets no colour rather than a
 *  guess — mis-lighting a source window is worse than leaving it
 *  plain, because it invites a reader to trust the wrong token. */
export function languageForPath(path: string | undefined): string | undefined {
  const ext = /\.([a-z0-9]+)(?:\?|$)/i.exec(path ?? '')?.[1]?.toLowerCase();
  switch (ext) {
    case 'ts':
    case 'tsx':
      return 'typescript';
    case 'js':
    case 'jsx':
    case 'mjs':
    case 'cjs':
      return 'javascript';
    case 'swift':
      return 'swift';
    case 'kt':
    case 'kts':
      return 'kotlin';
    case 'java':
      return 'java';
    case 'go':
      return 'go';
    case 'rs':
      return 'rust';
    case 'py':
      return 'python';
    case 'c':
    case 'cc':
    case 'cpp':
    case 'h':
    case 'hpp':
    case 'm':
    case 'mm':
      return 'cpp';
    case 'cs':
      return 'csharp';
    case 'json':
      return 'json';
    case 'sh':
      return 'bash';
    default:
      return undefined;
  }
}

/** One block, as HTML. Plain-escaped when the grammar is unknown. */
export function highlightBlock(code: string, language: string | undefined): string {
  if (language === undefined || !hljs.getLanguage(language)) return escapeHtml(code);
  // `ignoreIllegals`: a source window is a slice out of the middle of
  // a file, so it is routinely not a valid program. Without this the
  // grammar throws on the first unbalanced brace and the window loses
  // its colour entirely.
  return hljs.highlight(code, { ignoreIllegals: true, language }).value;
}

/** The same, split back into lines, each one balanced on its own.
 *
 *  The block is highlighted whole — that is the point, a `/* … *&#47;`
 *  spanning four rows is one comment — and then cut, re-opening on
 *  each new line whatever spans were still open at the cut. Splitting
 *  the *input* per line instead is what the old scanner did, and it
 *  is why a block comment read as code.
 */
export function highlightLines(code: string, language: string | undefined): string[] {
  const html = highlightBlock(code, language);
  const out: string[] = [];
  const open: string[] = [];
  let line = '';

  // One pass over the produced HTML: tags adjust the open stack, text
  // accumulates, and a newline in the *text* closes and reopens.
  const re = /(<[^>]+>)|([^<]+)/g;
  for (let m = re.exec(html); m !== null; m = re.exec(html)) {
    const [, tag, text] = m;
    if (tag !== undefined) {
      if (tag.startsWith('</')) open.pop();
      else if (!tag.endsWith('/>')) open.push(tag);
      line += tag;
      continue;
    }
    const parts = (text ?? '').split('\n');
    for (let i = 0; i < parts.length; i++) {
      if (i > 0) {
        out.push(line + '</span>'.repeat(open.length));
        line = open.join('');
      }
      line += parts[i];
    }
  }
  out.push(line + '</span>'.repeat(open.length));
  return out;
}

function escapeHtml(s: string): string {
  return s
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;');
}
