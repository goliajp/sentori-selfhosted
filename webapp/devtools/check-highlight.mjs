// A highlighter that drops a character is corrupting code somebody is
// about to paste into a terminal.
//
// So: strip the tags back off and compare with the input. Every
// snippet this console hands out, in its own grammar, plus the line
// splitter on a block comment spanning rows — the case the scanner
// this replaced could not do, and said so in its own comment.

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const { highlightBlock, highlightLines, languageForPath } = await import(
  join(root, 'src/lib/highlight.ts')
);
const snippets = await import(join(root, 'src/lib/push-snippets.ts'));

/** What the browser will read back out of the HTML. */
function strip(html) {
  return html
    .replace(/<[^>]+>/g, '')
    .replaceAll('&lt;', '<')
    .replaceAll('&gt;', '>')
    .replaceAll('&quot;', '"')
    .replaceAll('&#x27;', "'")
    .replaceAll('&amp;', '&');
}

/** Every `<span>` opened is closed, in order. */
function balanced(html) {
  let depth = 0;
  for (const m of html.matchAll(/<(\/?)span\b[^>]*>/g)) {
    depth += m[1] === '/' ? -1 : 1;
    if (depth < 0) return false;
  }
  return depth === 0;
}

const problems = [];
let checked = 0;

const HLJS_LANG = {
  node: 'typescript',
  csharp: 'csharp',
  cpp: 'cpp',
  go: 'go',
  java: 'java',
  python: 'python',
  rust: 'rust',
};

for (const { id } of snippets.SNIPPET_LANGS) {
  const code = snippets.snippet(id, 'https://example.test');
  const lang = HLJS_LANG[id];
  if (lang === undefined) {
    problems.push(`${id}: no grammar mapped`);
    continue;
  }
  const html = highlightBlock(code, lang);
  checked += 1;
  if (strip(html) !== code) problems.push(`${id}: highlighting changed the code`);
  if (!balanced(html)) problems.push(`${id}: unbalanced spans`);
  if (!html.includes('hljs-')) problems.push(`${id}: nothing was coloured`);
}

for (const [name, text] of [
  ['count', snippets.countSnippet('https://example.test')],
  ['poll', snippets.pollSnippet('https://example.test')],
]) {
  const html = highlightBlock(text, 'bash');
  checked += 1;
  if (strip(html) !== text) problems.push(`${name}: highlighting changed the code`);
  if (!balanced(html)) problems.push(`${name}: unbalanced spans`);
}

// The line splitter, on the shape that defeated the scanner this
// replaced: a comment that spans rows, and a string containing what
// looks like the start of one.
const window = [
  'function pay(order) {',
  '  /* the retry path is',
  '     three rows of comment */',
  '  const url = "https://x/* not a comment */"',
  '  return fetch(url)',
  '}',
].join('\n');

const lines = highlightLines(window, 'typescript');
checked += 1;
if (lines.length !== 6) problems.push(`line splitter produced ${lines.length} lines, not 6`);
if (strip(lines.join('\n')) !== window) problems.push('line splitter changed the code');
for (const [i, l] of lines.entries()) {
  if (!balanced(l)) problems.push(`line ${i + 1} is not balanced on its own`);
}
// Rows two and three are one comment. A per-line scanner lit row two
// and lost row three.
for (const i of [1, 2]) {
  if (!lines[i].includes('hljs-comment')) {
    problems.push(`line ${i + 1} of a block comment is not lit as one`);
  }
}

// An unknown extension gets no colour rather than a guess.
if (languageForPath('vendor/thing.wat') !== undefined) {
  problems.push('an unknown extension was guessed at');
}
if (languageForPath('src/checkout.ts') !== 'typescript') {
  problems.push('a .ts frame did not resolve to typescript');
}
const plain = highlightBlock('<script>alert(1)</script>', undefined);
if (plain.includes('<script>')) problems.push('unknown-language input was not escaped');

if (problems.length > 0) {
  console.error('✗ highlighting:\n');
  for (const p of problems) console.error(`    ${p}`);
  process.exit(1);
}
console.log(`✓ ${checked} blocks highlighted losslessly, spans balanced, block comments intact`);
