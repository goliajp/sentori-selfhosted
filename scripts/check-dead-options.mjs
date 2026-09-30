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
    // `.tsx` too. Without it, `rage-tap.tsx` — the only reader of
    // `detect.rageTap` outside init — was invisible, so an option read
    // solely from a component would have been reported dead.
    else if ((p.endsWith('.ts') || p.endsWith('.tsx')) && !p.endsWith('.d.ts')) out.push(p);
  }
  return out;
}

const files = ROOTS.flatMap((r) => walk(join(root, r)));
if (files.length === 0) {
  console.error(`✗ no sources under ${ROOTS.join(', ')} — this checker read nothing.`);
  process.exit(1);
}

/**
 * Comments removed before matching.
 *
 * Otherwise prose counts as a read: a line of documentation saying
 * `detect.uiThreadHang` satisfied the access pattern, so an option
 * that nothing implemented passed as long as something described it.
 * That is precisely the case this checker exists for — `linkHash` was
 * dead *and* had a confident doc comment explaining what it did.
 *
 * Whitespace replaces each comment rather than nothing, so the
 * "no `;` inside the braces" rule below still sees the real
 * punctuation around what it removed.
 */
const stripComments = (src) =>
  src
    .replace(/\/\*[\s\S]*?\*\//g, (m) => m.replace(/[^\n]/g, ' '))
    .replace(/(^|[^:])\/\/[^\n]*/g, (m, p1) => p1 + ' '.repeat(m.length - p1.length));

const sources = files.map((f) => ({ f, src: stripComments(readFileSync(f, 'utf8')) }));

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

/**
 * Field paths in a type body, descending into a field whose type is
 * written inline as another object.
 *
 * It used to collect the top level only, so `InitConfig.detect` counted
 * as one option and the four switches inside it as none — and those are
 * the ones a host actually sets. A dead `detect.slowApi` would have
 * read as covered for as long as `detect` itself was accessed
 * somewhere.
 */
function collectFields(body, prefix = '') {
  const out = [];
  const lines = body.split('\n');
  let depth = 0;
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i];
    const trimmed = line.trim();
    if (depth === 0) {
      const m = /^(\w+)\??\s*:(.*)$/.exec(trimmed);
      if (m) {
        const name = prefix + m[1];
        // An inline object opens here and its fields belong to it, not
        // to the parent.
        if (m[2].trim().startsWith('{')) {
          let d = 0;
          const inner = [];
          for (let j = i; j < lines.length; j += 1) {
            d += (lines[j].match(/[{]/g) ?? []).length - (lines[j].match(/[}]/g) ?? []).length;
            inner.push(j === i ? lines[j].slice(lines[j].indexOf('{') + 1) : lines[j]);
            if (d === 0) break;
          }
          out.push(...collectFields(inner.join('\n'), `${name}.`));
        } else {
          out.push(name);
        }
      }
    }
    depth += (line.match(/[{[(]/g) ?? []).length - (line.match(/[}\])]/g) ?? []).length;
  }
  return out;
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
  const fields = collectFields(body);

  for (const path of fields) {
    checked += 1;
    // The last segment is what an access looks like in the code:
    // `config.detect?.slowApi` reads as `.slowApi`.
    const field = path.split('.').pop();
    // Reading an option means accessing it — `opts.field`, or pulling
    // it out of a destructuring pattern.
    //
    // The first version counted "mentions minus declarations" and
    // called `onMessage` and `onTap` dead. A function whose parameter
    // is typed `PushRegisterOptions['onMessage']` has a line that
    // looks exactly like a field declaration, which excluded the very
    // file doing the reading. Match the access, not its shadow.
    const access = new RegExp(`\\.\\s*${field}\\b`);
    // No `;` inside the braces. With `[^{}]*` this spanned a whole
    // file: a type declaration listing the field, several statements
    // later, and any `} =` after it all counted as one destructuring
    // pattern — so a field that only ever appeared in its own type
    // declaration read as used, which is the exact case this checker
    // is for.
    const destructured = new RegExp(`\\{[^{};]*\\b${field}\\b[^{};]*\\}\\s*(?::[^=;]+)?=`);
    const used = sources.some(
      ({ f, src }) => !f.includes('__tests__') && (access.test(src) || destructured.test(src)),
    );
    if (!used) {
      problems.push(
        `${typeName}.${path} is declared in ${holder.f.replace(`${root}/`, '')} and read nowhere — ` +
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
