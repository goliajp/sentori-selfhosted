// A relative timestamp with nothing behind it.
//
// `formatRelative` has carried a comment since it was written saying
// the exact timestamp lives in the title attribute wherever it is
// rendered. It lived in none of them: twenty-five call sites, zero
// titles, so an audit log whose rows all read "3 minutes ago" could not
// do the one thing an audit log is for, and a device with a wrong clock
// showed its own opinion of when something happened with no way to see
// ours.
//
// The rule: render a moment through `<TimeAgo>`, which carries both.
// Calling `formatRelative` directly is allowed only where a title has
// nowhere to live — inside an attribute, a chart label, a string being
// concatenated — and those sites carry a `// bare-relative: <why>`
// comment on the line above, where the reason is readable rather than
// crammed in after the code.
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join } from 'node:path';

const ROOT = new URL('../webapp/src/', import.meta.url).pathname;
const files = [];
const walk = (d) => {
  for (const e of readdirSync(join(ROOT, d))) {
    const rel = d ? `${d}/${e}` : e;
    if (statSync(join(ROOT, rel)).isDirectory()) walk(rel);
    else if (/\.tsx?$/.test(e)) files.push(rel);
  }
};
walk('');
if (files.length < 20) {
  console.error(`✗ scanned ${files.length} files. Broken checker, not a broken tree.`);
  process.exit(1);
}

const bare = [];
for (const rel of files) {
  if (rel.endsWith('components/ui.tsx')) continue; // where both live
  const src = readFileSync(join(ROOT, rel), 'utf8');
  const lines = src.split('\n');
  lines.forEach((line, i) => {
    if (!line.includes('formatRelative(')) return;
    if (line.includes('bare-relative:')) return;
    if ((lines[i - 1] ?? '').includes('bare-relative:')) return;
    bare.push(`${rel}:${i + 1}  ${line.trim().slice(0, 78)}`);
  });
}

if (bare.length) {
  console.error(`✗ ${bare.length} timestamp(s) rendered with no absolute time behind them:`);
  for (const b of bare.slice(0, 12)) console.error(`    ${b}`);
  if (bare.length > 12) console.error(`    … and ${bare.length - 12} more`);
  console.error(`  Use <TimeAgo iso={…} />, or put \`// bare-relative: <why>\` above it.`);
  process.exit(1);
}
console.log(`✓ every rendered moment carries its absolute time (${files.length} files)`);
