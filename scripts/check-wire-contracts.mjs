// The unions the SDK writes and the server reads, checked against
// each other.
//
// These pairs are maintained by hand across a language boundary, and
// nothing compiles them together. A kind added on one side and
// forgotten on the other is not a type error anywhere — it is an
// event rejected at ingest, from a device, into a log.
//
//   node scripts/check-wire-contracts.mjs

import { readFileSync } from 'node:fs';

const TYPES = 'sdk/core/src/types.ts';
const PIPELINE = 'self-hosted/server/src/pipeline.rs';
const ATTACHMENTS = 'self-hosted/server/src/handlers/sdk/events_attachments.rs';
const INGEST = 'self-hosted/server/src/handlers/sdk/events.rs';
const SWIFT_ATT = 'sdk/native/ios/Sources/Sentori/SentoriAttachment.swift';
const KOTLIN_ATT = 'sdk/native/android/src/main/java/com/sentori/SentoriAttachment.kt';

const problems = [];

/** `export type X = 'a' | 'b'` or the multi-line `| 'a'` form. */
function tsUnion(src, name) {
  const m = new RegExp(`export type ${name} =([^;]*?)\\n\\n`, 's').exec(`${src}\n\n`);
  if (!m) return null;
  const values = [...m[1].matchAll(/'([^']+)'/g)].map((x) => x[1]);
  return values.length > 0 ? values.sort() : null;
}

/** `pub enum Kind { Error, Warn, ... }` → lowercased variants. */
function rustEnum(src, name) {
  const m = new RegExp(`pub enum ${name} \\{([^}]*)\\}`, 's').exec(src);
  if (!m) return null;
  const values = [...m[1].matchAll(/^\s*([A-Z]\w*)\s*,/gm)].map((x) =>
    x[1].toLowerCase(),
  );
  return values.length > 0 ? values.sort() : null;
}

/** `const KINDS: [&str; N] = ["a", "b"];` */
function rustStrArray(src, name) {
  const m = new RegExp(`(?:pub )?const ${name}: \\[&str; \\d+\\] = \\[([^\\]]*)\\]`, 's').exec(src);
  if (!m) return null;
  const values = [...m[1].matchAll(/"([^"]+)"/g)].map((x) => x[1]);
  return values.length > 0 ? values.sort() : null;
}

const types = readFileSync(TYPES, 'utf8');
const pipeline = readFileSync(PIPELINE, 'utf8');
const attachments = readFileSync(ATTACHMENTS, 'utf8');
const ingest = readFileSync(INGEST, 'utf8');

// ── the five kinds: both directions matter ────────────────────────
// An event kind the server does not know is rejected at ingest; one
// the SDK cannot name is a feature with no way to reach it.
const eventKinds = tsUnion(types, 'EventKind');
const serverKinds = rustEnum(pipeline, 'Kind');
if (!eventKinds || !serverKinds) {
  problems.push(
    `could not read EventKind (${TYPES}) or enum Kind (${PIPELINE}) — ` +
      'one of them moved or changed shape. A checker that parses nothing must not pass.',
  );
} else {
  for (const k of eventKinds) {
    if (!serverKinds.includes(k)) {
      problems.push(`EventKind has '${k}'; ${PIPELINE} enum Kind does not — ingest would reject it`);
    }
  }
  for (const k of serverKinds) {
    if (!eventKinds.includes(k)) {
      problems.push(`enum Kind has '${k}'; ${TYPES} EventKind does not — no SDK can send it`);
    }
  }
}

// ── attachment kinds: subset, not equality ────────────────────────
// The server's list mirrors a database CHECK constraint and may
// outlive a kind the SDK has retired (`sessionTrail` did). What must
// never happen is the other direction: an SDK that can name a kind
// the database will refuse.
const attachmentKinds = tsUnion(types, 'AttachmentKind');
const serverAttachmentKinds = rustStrArray(attachments, 'KINDS');
if (!attachmentKinds || !serverAttachmentKinds) {
  problems.push(
    `could not read AttachmentKind (${TYPES}) or KINDS (${ATTACHMENTS}) — ` +
      'one of them moved or changed shape.',
  );
} else {
  for (const k of attachmentKinds) {
    if (!serverAttachmentKinds.includes(k)) {
      problems.push(
        `AttachmentKind has '${k}'; ${ATTACHMENTS} KINDS does not — the upload ` +
          'is refused, and the CHECK constraint behind it would refuse the row too',
      );
    }
  }
}

// ── the native allowlists ─────────────────────────────────────────
//
// The native SDKs filter before posting, which saves a round trip and
// costs an attachment when the two lists disagree. The first version
// of the Swift list held three kinds; the crash handler on both
// platforms writes `viewTree`, which was not one of them — so the
// delivery path would have dropped, silently and on the way out, the
// evidence it exists to deliver. The event still arrives, so nothing
// looks wrong; the viewport is just empty.
//
// Checked in both directions. Missing a kind loses evidence; carrying
// one the server refuses spends a device's bandwidth on a 400.
for (const [label, path] of [
  ['Swift', SWIFT_ATT],
  ['Kotlin', KOTLIN_ATT],
]) {
  const src = readFileSync(path, 'utf8');
  const listed = [...src.matchAll(/"([a-zA-Z]+)"/g)]
    .map((m) => m[1])
    .filter((v) => serverAttachmentKinds?.includes(v));
  const native = [...new Set(listed)].sort();
  if (native.length === 0) {
    problems.push(`could not read an attachment allowlist out of ${path}`);
    continue;
  }
  for (const k of serverAttachmentKinds ?? []) {
    if (!native.includes(k)) {
      problems.push(
        `${ATTACHMENTS} accepts '${k}'; the ${label} allowlist (${path}) does ` +
          'not — that attachment is dropped by the SDK before it is sent, and ' +
          'the event arrives looking as though nothing was captured',
      );
    }
  }
}

// ── platforms: equality, both directions matter ───────────────
//
// Platform is part of the issue fingerprint, so it is not a label the
// two sides can drift on quietly. A value the SDK can name and the
// server cannot is no longer a 400 — it is stored as `unknown`, which
// means every platform the mismatch touches collapses into one issue
// and nothing says so except a counter. A value the server knows and
// no SDK can name is a runtime nobody can report from.
//
// `unknown` is not in either list on purpose: it is where the server
// puts what it does not recognise, never something an SDK sends.
const platforms = tsUnion(types, 'Platform');
const serverPlatforms = rustStrArray(ingest, 'VALID_PLATFORMS');
if (!platforms || !serverPlatforms) {
  problems.push(
    `could not read Platform (${TYPES}) or VALID_PLATFORMS (${INGEST}) — ` +
      'one of them moved or changed shape. A checker that parses nothing must not pass.',
  );
} else {
  if (serverPlatforms.includes('unknown')) {
    problems.push(
      `${INGEST} VALID_PLATFORMS lists 'unknown' — that is the value the server ` +
        'substitutes for what it does not recognise, so listing it as acceptable ' +
        'input lets a client pin every event to the collapsed bucket on purpose',
    );
  }
  for (const p of platforms) {
    if (!serverPlatforms.includes(p)) {
      problems.push(
        `Platform has '${p}'; ${INGEST} VALID_PLATFORMS does not — events from ` +
          "that runtime are stored as 'unknown' and share one fingerprint bucket",
      );
    }
  }
  for (const p of serverPlatforms) {
    if (!platforms.includes(p)) {
      problems.push(
        `VALID_PLATFORMS has '${p}'; ${TYPES} Platform does not — no SDK can name it`,
      );
    }
  }
}

// ── attachment sources ────────────────────────────────────────────
const sources = tsUnion(types, 'AttachmentSource');
const serverSources = rustStrArray(attachments, 'SOURCES');
if (sources && serverSources) {
  for (const s of sources) {
    if (!serverSources.includes(s)) {
      problems.push(`AttachmentSource has '${s}'; ${ATTACHMENTS} SOURCES does not`);
    }
  }
}

if (problems.length === 0) {
  console.log(
    `✓ wire contracts agree: ${eventKinds.length} event kinds, ` +
      `${attachmentKinds.length} attachment kinds, ${platforms.length} platforms`,
  );
  process.exit(0);
}
for (const p of problems) console.error(`✗ ${p}`);
console.error(
  '\nThese unions are maintained by hand across a language boundary and\n' +
    'nothing compiles them together. A mismatch is not a type error — it\n' +
    'is an event rejected at ingest, on a device, into a log.',
);
process.exit(1);
