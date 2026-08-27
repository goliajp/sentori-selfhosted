#!/usr/bin/env node
// An option the public types advertise must be read by the code.
//
// `PushRegisterOptions` declared two that nothing touched:
//
//   linkHash  — left over from an identity design that was removed
//               when `user_fingerprint_hex` became `user_key`. Its
//               doc comment still said it was how the server targets
//               a specific user across their devices.
//   metadata  — never put in the request body, no field on the
//               server's `RegisterBody` to receive it, while
//               `device_tokens.metadata` sat at '{}' since the table
//               was created.
//
// A dead option is worse than a missing one. Missing, the integrator
// asks. Present with a confident comment, they use it and get a
// working call with a silently wrong result — insight passed
// `linkHash` instead of calling `sentori.user()`, saw `ok: true` and
// a rising device count, and had no way to learn why "addressable"
// stayed at zero (2026-08-11).
//
//   node scripts/check-dead-options.mjs

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

/** Exported option bags whose fields the host is invited to set. Add
 *  a type here when a new public options object appears. */
const TYPES = ['PushRegisterOptions', 'InitConfig', 'ReplayOptions'];

const ROOTS = ['sdk/react-native/src', 'sdk/core/src', 'sdk/expo/src'];

function walk(dir, out = []) {
  let entries;
  try {
    entries = readdirSync(dir);
  } catch {
    return out;
  }
  for (const e of entries) {
    const p = join(dir, e);
    if (statSync(p).isDirectory()) walk(p, out);
    else if (p.endsWith('.ts') && !p.endsWith('.d.ts')) out.push(p);
  }
  return out;
}

const files = ROOTS.flatMap((r) => walk(join(root, r)));
if (files.length === 0) {
  console.error(`✗ no sources under ${ROOTS.join(', ')} — this checker read nothing.`);
  process.exit(1);
}

const sources = files.map((f) => ({ f, src: readFileSync(f, 'utf8') }));

/** Body of `export type NAME = { … }`, bracket-matched so nested
 *  object fields do not end it early. */
function typeBody(src, name) {
  const m = new RegExp(`export type ${name}\\s*=\\s*\\{`).exec(src);
  if (!m) return null;
  let depth = 0;
  for (let i = m.index + m[0].length - 1; i < src.length; i += 1) {
    if (src[i] === '{') depth += 1;
    else if (src[i] === '}') {
      depth -= 1;
      if (depth === 0) return src.slice(m.index + m[0].length, i);
    }
  }
  return null;
}

const problems = [];
let checked = 0;

for (const typeName of TYPES) {
  const holder = sources.find(({ src }) => typeBody(src, typeName) !== null);
  if (!holder) {
    problems.push(`type ${typeName} is listed here but no longer exists — this list is stale`);
    continue;
  }
  const body = typeBody(holder.src, typeName);
  // Top-level field names only: skip anything nested inside a field's
  // own object literal, which `depth` tracks.
  const fields = [];
  let depth = 0;
  for (const line of body.split('\n')) {
    const trimmed = line.trim();
    if (depth === 0) {
      const m = /^(\w+)\??\s*:/.exec(trimmed);
      if (m) fields.push(m[1]);
    }
    depth += (line.match(/[{[(]/g) ?? []).length - (line.match(/[}\])]/g) ?? []).length;
  }

  for (const field of fields) {
    checked += 1;
    // Reading an option means accessing it — `opts.field`, or pulling
    // it out of a destructuring pattern.
    //
    // The first version counted "mentions minus declarations" and
    // called `onMessage` and `onTap` dead. A function whose parameter
    // is typed `PushRegisterOptions['onMessage']` has a line that
    // looks exactly like a field declaration, which excluded the very
    // file doing the reading. Match the access, not its shadow.
    const access = new RegExp(`\\.\\s*${field}\\b`);
    const destructured = new RegExp(`\\{[^{}]*\\b${field}\\b[^{}]*\\}\\s*(?::[^=]+)?=`);
    const used = sources.some(
      ({ f, src }) => !f.includes('__tests__') && (access.test(src) || destructured.test(src)),
    );
    if (!used) {
      problems.push(
        `${typeName}.${field} is declared in ${holder.f.replace(`${root}/`, '')} and read nowhere — ` +
          `a host that sets it gets a call that succeeds and does nothing`,
      );
    }
  }
}

if (checked === 0) {
  console.error('✗ no option fields were checked — this checker read nothing.');
  process.exit(1);
}
if (problems.length === 0) {
  console.log(`✓ ${checked} public options across ${TYPES.length} types are read by the code`);
  process.exit(0);
}
for (const p of problems) console.error(`✗ ${p}`);
console.error(
  '\nWire the option up, or delete it. Leaving it is the worst of the three:\n' +
    'it reads like the answer, and the call it appears in still succeeds.',
);
process.exit(1);
