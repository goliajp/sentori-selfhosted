// An audit row answers four questions or it is not an audit row.
//
//   node devtools/check-audit-columns.mjs
//
// Who, when, on what, and what they did. The table shipped with three
// of them: the target column rendered eight characters of a uuid and
// nothing else, so a row read "someone deleted 01a0ef95" — a project,
// a token, a person, no way to tell. The server had been returning
// `targetType` the whole time.
//
// A static read of the column definitions, because the alternative is
// a render test that only fires when someone remembers to look at this
// tab, and this tab is the one nobody opens until an auditor asks.

import { readFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const src = readFileSync(join(root, 'src/pages/Settings.tsx'), 'utf8');

// The audit table's column list, from `function AuditTab` to the end
// of its columns array.
const tab = src.slice(src.indexOf('function AuditTab'));
if (!tab || tab.length === src.length) {
  console.error('✗ AuditTab has moved — this check now reads nothing');
  process.exit(1);
}

const REQUIRED = [
  ['r.createdAt', 'when it happened'],
  ['r.actorEmail', 'who did it'],
  ['r.action', 'what they did'],
  ['r.targetType', 'what kind of thing they did it to'],
  ['r.targetId', 'which one'],
];

const missing = REQUIRED.filter(([field]) => !tab.includes(field));
if (missing.length > 0) {
  console.error('✗ the audit table does not say:');
  for (const [field, why] of missing) console.error(`    ${why}  (${field})`);
  console.error('\nAn audit log missing one of these is a list of events, not an audit.');
  process.exit(1);
}

// The full id has to be reachable. Eight characters cannot be pasted
// into a query and two objects can share a prefix.
if (!/title=\{r\.targetId\}/.test(tab)) {
  console.error('✗ the audit table truncates the target id with no way to read the whole one');
  process.exit(1);
}

console.log(`✓ audit rows say all ${REQUIRED.length} things an audit row has to say`);
