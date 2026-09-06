// A mapping uploaded after the crash still has to rescue the crash.
//
//   SENTORI_BASE=… SENTORI_TOKEN=… SENTORI_API_TOKEN=… SENTORI_JAR=… \
//     node scripts/check-symbolication.mjs
//
// The three resolver crates have had unit tests and benchmarks since
// before the v0.2 cutover. What none of them touched is the chain:
// upload → store → find by release → rewrite the stored payload. The
// four bugs fixed on 2026-08-10 were all in that chain, and every one
// of them left the resolvers' own tests green.
//
// The property under test is the one the iron rule depends on. A
// build-time upload is allowed to fail without failing the build —
// which is only safe because a later upload re-reads the crashes that
// arrived while the mapping was missing. Without that, "allowed to
// fail" means "allowed to lose the stack permanently", and nothing
// asserted it happened.
import { execFileSync } from 'node:child_process';

const BASE = process.env.SENTORI_BASE;
const TOKEN = process.env.SENTORI_TOKEN;
const API_TOKEN = process.env.SENTORI_API_TOKEN;
const JAR = process.env.SENTORI_JAR;
if (!BASE || !TOKEN || !API_TOKEN || !JAR) {
  console.error('set SENTORI_BASE, SENTORI_TOKEN, SENTORI_API_TOKEN and SENTORI_JAR');
  process.exit(2);
}

const fail = (m) => { console.error(`✗ ${m}`); process.exit(1); };

// Two methods, one with a single line and one with a range, so a
// resolver that only handles exact hits is caught by the second.
const MAPPING = [
  '# compiler: R8',
  '# compiler_version: 8.5.0',
  'com.example.feature.SearchPresenter -> p.q.r:',
  '    void onQuery() -> a',
  '    100:100:void onQuery():100:100 -> a',
  '    void formatResults() -> b',
  '    200:210:void formatResults():200:210 -> b',
  '',
].join('\n');

const RELEASE = `sym-e2e@1.0.0+${process.hrtime.bigint()}`;

const send = async (release) => {
  const r = await fetch(`${BASE}/v1/events`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${TOKEN}` },
    body: JSON.stringify({
      kind: 'error',
      occurredAt: new Date().toISOString(),
      platform: 'android',
      release,
      name: 'SearchPresenter crash',
      // The error rides inside `payload` — the routing columns are
      // the only things hoisted to the top level.
      payload: {
        error: {
          type: 'java.lang.IllegalStateException',
          message: 'query ran on a closed presenter',
          stack: [
            { function: 'p.q.r.a', line: 100 },
            { function: 'p.q.r.b', line: 205 },
            // No mapping covers this one. A resolver that answers for
            // everything is worse than one that answers for nothing:
            // the frame reads as source and points nowhere.
            { function: 'x.y.z.unmapped', line: 7 },
          ],
        },
      },
    }),
  });
  if (r.status !== 202) fail(`ingest answered ${r.status} for a plain android crash`);
  return (await r.json()).eventId;
};

const readStack = (eventId) => {
  const raw = execFileSync('curl', ['-s', '-b', JAR, `${BASE}/admin/api/events/${eventId}`],
    { encoding: 'utf8' });
  let body;
  try { body = JSON.parse(raw); } catch { fail(`could not read event: ${raw.slice(0, 160)}`); }
  const stack = body?.payload?.error?.stack ?? body?.error?.stack;
  if (!Array.isArray(stack)) fail(`event ${eventId} came back without a stack: ${raw.slice(0, 200)}`);
  return stack;
};

const upload = (token, release) =>
  execFileSync('curl', ['-s', '-o', '/dev/null', '-w', '%{http_code}',
    '-X', 'POST', `${BASE}/v1/releases/${encodeURIComponent(release)}/artifacts`,
    '-H', `authorization: Bearer ${token}`,
    '-F', 'kind=proguard',
    '-F', 'file=@-;filename=mapping.txt'],
    { encoding: 'utf8', input: MAPPING });

// ── 1. The crash arrives before the mapping does, which is the normal
// order: the app ships, it crashes, and the upload failed or ran late.
const beforeId = await send(RELEASE);
const before = readStack(beforeId);
if (before.some((f) => f.symbolicated)) {
  fail('a frame was already symbolicated with no mapping uploaded — ' +
       'the rest of this check would prove nothing');
}

// ── 2. Only an `api` token may replace what stacks resolve to. The
// token that ships inside the customer's app must not: anyone with the
// APK could otherwise point every frame wherever they liked, and a
// wrong mapping looks exactly like a right one.
const forbidden = upload(TOKEN, RELEASE);
if (forbidden !== '403') {
  fail(`an ingest-scope token got ${forbidden} uploading a mapping, expected 403`);
}

// ── 3. The real upload.
const code = upload(API_TOKEN, RELEASE);
if (code !== '201' && code !== '200') fail(`uploading the mapping answered ${code}`);

// ── 4. The retro pass runs detached from the response, so poll.
let after = null;
for (let i = 0; i < 40; i++) {
  await new Promise((r) => setTimeout(r, 250));
  after = readStack(beforeId);
  if (after[0]?.symbolicated) break;
}
if (!after?.[0]?.symbolicated) {
  fail('the crash that predated the mapping was never re-read — ' +
       'a failed build-time upload loses the stack for good');
}

const expect = (frame, fn, line, label) => {
  if (frame.function !== fn) fail(`${label}: function is ${frame.function}, expected ${fn}`);
  if (line !== null && frame.line !== line) fail(`${label}: line is ${frame.line}, expected ${line}`);
};

expect(after[0], 'com.example.feature.SearchPresenter.onQuery', 100, 'retro frame 0');
if (after[0].minifiedFunction !== 'p.q.r.a') {
  fail(`retro frame 0 lost the minified name: ${after[0].minifiedFunction}`);
}
// 205 sits inside 200:210, so a range lookup is what resolves it.
expect(after[1], 'com.example.feature.SearchPresenter.formatResults', null, 'retro frame 1');
if (after[2].symbolicated || after[2].function !== 'x.y.z.unmapped') {
  fail(`an unmapped frame was rewritten to ${after[2].function} — invented source`);
}

// ── 5. With the mapping in place, the next crash resolves on the way in.
const liveId = await send(RELEASE);
const live = readStack(liveId);
if (!live[0]?.symbolicated) fail('a crash arriving after the upload was not symbolicated at ingest');
expect(live[0], 'com.example.feature.SearchPresenter.onQuery', 100, 'live frame 0');

// ── 6. A mapping belongs to one release. Applying it to another would
// resolve names that happen to collide across builds — obfuscated
// names are short and reused, so `p.q.r.a` exists in most of them.
const otherId = await send(`${RELEASE}-other`);
const other = readStack(otherId);
if (other[0]?.symbolicated) {
  fail('a mapping resolved a crash from a different release');
}

// ── 7. A dSYM is selected by the debug id in its stored name, and the
// id lives in the file rather than in what the uploader called it.
// The CLI writes a name that carries it; the manual path this repo's
// own hint offers ("the binary inside the .dSYM bundle,
// Contents/Resources/DWARF/<name>") writes a name that does not. That
// upload parsed, stored, and listed as usable, and no frame could
// ever resolve against it.
const machO = (uuid) => {
  const h = Buffer.alloc(32);
  h.writeUInt32LE(0xfeedfacf, 0); // MH_MAGIC_64
  h.writeUInt32LE(0x0100000c, 4); // CPU_TYPE_ARM64
  h.writeUInt32LE(0, 8);
  h.writeUInt32LE(10, 12); // MH_DSYM
  h.writeUInt32LE(uuid ? 1 : 0, 16); // ncmds
  h.writeUInt32LE(uuid ? 24 : 0, 20); // sizeofcmds
  h.writeUInt32LE(0, 24);
  h.writeUInt32LE(0, 28);
  if (!uuid) return h;
  const lc = Buffer.alloc(24);
  lc.writeUInt32LE(0x1b, 0); // LC_UUID
  lc.writeUInt32LE(24, 4);
  uuid.copy(lc, 8);
  return Buffer.concat([h, lc]);
};

const uploadFile = (release, kind, filename, bytes) => {
  const raw = execFileSync('curl', ['-s',
    '-X', 'POST', `${BASE}/v1/releases/${encodeURIComponent(release)}/artifacts`,
    '-H', `authorization: Bearer ${API_TOKEN}`,
    '-F', `kind=${kind}`,
    '-F', `file=@-;filename=${filename}`],
    { encoding: 'buffer', input: bytes });
  const text = raw.toString('utf8');
  try { return JSON.parse(text); } catch { fail(`upload did not answer JSON: ${text.slice(0, 200)}`); }
};

const ID = 'A1B2C3D4E5F60718293A4B5C6D7E8F90';
const dsymRelease = `${RELEASE}-ios`;
// Named after the product, exactly as the bundle's inner binary is.
const stored = uploadFile(dsymRelease, 'dsym', 'MyApp', machO(Buffer.from(ID, 'hex')));

if (!Array.isArray(stored.debugIds) || !stored.debugIds.includes(ID)) {
  fail(`the debug id was not read out of the file: ${JSON.stringify(stored.debugIds)}`);
}
if (!stored.name.toUpperCase().includes(ID)) {
  fail(`stored as "${stored.name}" — no frame carrying ${ID} can find it`);
}
if (stored.debugId !== ID) fail(`debugId is ${stored.debugId}, expected ${ID}`);
if (stored.usable !== true) fail(`a dSYM with an LC_UUID reported usable=${stored.usable}`);

// A name that already carries the id is left alone — `sentori-cli`
// writes one, and appending a second copy would be this fix making a
// mess of the path that was already correct.
const cliNamed = uploadFile(dsymRelease, 'dsym', `MyApp-arm64-${ID}`,
  machO(Buffer.from(ID, 'hex')));
if (cliNamed.name !== `MyApp-arm64-${ID}`) {
  fail(`a name that already carried the id was rewritten to "${cliNamed.name}"`);
}

// ── 8. And one that genuinely cannot serve anything must say so
// rather than report itself fine. Selection is by debug id; a Mach-O
// carrying none is unreachable however well it parses.
const idless = uploadFile(dsymRelease, 'dsym', 'NoUuid', machO(null));
if (idless.usable !== false) {
  fail(`a Mach-O with no LC_UUID reported usable=${idless.usable} — ` +
       'the console would call an upload fine that no frame can reach');
}
if (!/LC_UUID/.test(idless.hint ?? '')) {
  fail(`the hint does not say why: ${idless.hint}`);
}

console.log('✓ symbolication holds: retro rescue, ingest-time, scope, release isolation, dSYM identity');
