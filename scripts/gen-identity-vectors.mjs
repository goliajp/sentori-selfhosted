#!/usr/bin/env node
// Regenerate the identity vectors the native SDKs assert against.
//
// `userKey` is the one value three implementations have to agree on:
// TypeScript computes it today, and Swift and Kotlin are about to.
// If they ever disagree, a device stops matching the events from the
// same person — push reaches nobody, and nothing anywhere reports it.
// That silence is exactly why insight declined to write their own
// client against the HTTP contract (2026-08-11).
//
// So the vectors are generated *by* the source of truth rather than
// written beside it. `sdk/core/src/identity.ts` decides what gets
// normalised and how; this file only records what it decided.
//
//   node scripts/gen-identity-vectors.mjs           write
//   node scripts/gen-identity-vectors.mjs --check   fail if stale
//
// Build `@goliapkg/sentori-core` first — this imports the compiled
// module, not the source.

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = join(root, 'sdk/native/fixtures/identity-vectors.json');
const check = process.argv.includes('--check');

const { hashIdentities } = await import(join(root, 'sdk/core/lib/identity.js'));

// One case per branch of `normalise`, plus the shapes that have
// historically broken hashing in one language and not another:
// surrounding whitespace, mixed case, non-ASCII, and a value long
// enough to cross a buffer boundary.
const CASES = [
  ['id', 'usr_123'],
  ['id', '  usr_123  '],
  ['email', 'A@B.com'],
  ['email', '  user@Example.COM  '],
  ['phone', '+81 (90) 1234-5678'],
  ['phone', '+819012345678'],
  ['username', '  DoraCawl '],
  ['googleSub', '  108bc  '],
  ['custom', '  trimmed  '],
  ['id', '日本語ユーザー'],
  ['id', 'e'.repeat(512)],
  // Whitespace no two of the three languages agree on by default.
  // ECMAScript's `WhiteSpace` production lists both U+00A0 and
  // U+FEFF. Swift's `.whitespacesAndNewlines` has the first and not
  // the second — Unicode calls a byte-order mark a format character.
  // Kotlin's `Character.isWhitespace` has neither. All three needed
  // an explicit set, and these two vectors are the only reason anyone
  // found that out: both native implementations were written, read
  // and believed correct first.
  ['id', '\u00a0usr_123\u00a0'],
  ['email', '\ufeff  A@B.com \u00a0'],
];

const vectors = [];
for (const [keyType, raw] of CASES) {
  const h = await hashIdentities({ [keyType]: raw });
  const sha256Hex = h[keyType];
  if (typeof sha256Hex !== 'string' || sha256Hex.length !== 64) {
    console.error(`✗ ${keyType} / ${JSON.stringify(raw)} produced ${sha256Hex}`);
    process.exit(1);
  }
  vectors.push({ keyType, raw, sha256Hex });
}

const body =
  JSON.stringify(
    {
      note:
        'Generated from sdk/core/src/identity.ts — the source of truth for what gets ' +
        'hashed and how. Regenerate with scripts/gen-identity-vectors.mjs; never hand-edit.',
      vectors,
    },
    null,
    2,
  ) + '\n';

if (!check) {
  writeFileSync(out, body);
  console.log(`✓ wrote ${vectors.length} identity vectors`);
  process.exit(0);
}

let have = null;
try {
  have = readFileSync(out, 'utf8');
} catch {
  // absent — reported below
}
if (have === body) {
  console.log(`✓ ${vectors.length} identity vectors match sdk/core/src/identity.ts`);
  process.exit(0);
}
console.error('✗ sdk/native/fixtures/identity-vectors.json is stale.');
console.error(
  '\nThe normalisation or hashing in sdk/core/src/identity.ts changed. Every native\n' +
    'SDK asserts against these vectors, so regenerate them and make the native\n' +
    'implementations agree — a userKey that differs by platform silently stops\n' +
    'matching a device to the person whose events it shares.',
);
process.exit(1);
