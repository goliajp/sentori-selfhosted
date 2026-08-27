// `/v1/events/validate` must agree with `/v1/events`.
//
//   SENTORI_BASE=… SENTORI_TOKEN=… node scripts/check-validate-agrees.mjs
//
// A validator that accepts a body ingest rejects is worse than no
// validator: it is a second source of truth that reads as
// authoritative and sends people to write integrations that fail in
// production instead of at the desk. The two share `WireEvent` and the
// platform list precisely so they cannot disagree, and this is the
// assertion that they do not.
//
// Runs in the e2e, where a real token exists.
const BASE = process.env.SENTORI_BASE;
const TOKEN = process.env.SENTORI_TOKEN;
if (!BASE || !TOKEN) {
  console.error('set SENTORI_BASE and SENTORI_TOKEN');
  process.exit(2);
}

const now = () => new Date().toISOString();
const CASES = [
  ['minimal, valid', { kind: 'trace', occurredAt: now(), platform: 'android', name: 'probe.min' }],
  ['trace without name', { kind: 'trace', occurredAt: now(), platform: 'ios' }],
  ['warn without name', { kind: 'warn', occurredAt: now(), platform: 'ios' }],
  ['probe without ref', { kind: 'probe', occurredAt: now(), platform: 'ios' }],
  ['error without name is fine', {
    kind: 'error', occurredAt: now(), platform: 'ios',
    payload: { error: { type: 'TypeError', message: 'agreement probe' } },
  }],
  ['full, valid', {
    kind: 'trace', occurredAt: now(), platform: 'ios', name: 'probe.full',
    release: 'app@1.0.0+1', environment: 'test',
    payload: { note: 'agreement probe' },
  }],
  ['unknown keys at top level', {
    kind: 'trace', occurredAt: now(), platform: 'ios', name: 'probe.unknown',
    error: { message: 'hoisted' }, device: { os: 'ios' },
  }],
  ['timestamp instead of occurredAt', {
    kind: 'trace', timestamp: now(), platform: 'ios',
  }],
  ['kind outside the enum', { kind: 'fatal', occurredAt: now(), platform: 'ios' }],
  ['platform outside the enum', { kind: 'trace', occurredAt: now(), platform: 'iOS' }],
  ['occurredAt not rfc3339', { kind: 'trace', occurredAt: 'yesterday', platform: 'ios' }],
  ['missing platform', { kind: 'trace', occurredAt: now() }],
  ['empty object', {}],
];

const post = (path, body, auth) =>
  fetch(`${BASE}${path}`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      ...(auth ? { authorization: `Bearer ${TOKEN}` } : {}),
    },
    body: JSON.stringify(body),
  });

// A dead token answers 401 to every body, which looks exactly like
// "ingest rejects everything the validator accepts". Prove the token
// works before believing a single comparison.
{
  const [, probe] = CASES[0];
  const r = await post('/v1/events', { ...probe, occurredAt: now() }, true);
  if (r.status !== 202) {
    const body = await r.text().catch(() => '');
    console.error(
      `✗ the token cannot ingest a known-good body (${r.status}). This ` +
        `harness is broken, not the tree — every comparison below would ` +
        `report a disagreement that is really an auth failure.`,
    );
    console.error(`    ${body.slice(0, 160)}`);
    process.exit(2);
  }
}

let disagreed = 0;
for (const [name, body] of CASES) {
  const v = await post('/v1/events/validate', body, false);
  const vj = await v.json().catch(() => ({}));
  const i = await post('/v1/events', body, true);

  const validatorSaysOk = v.status === 200 && vj.ok === true;
  const ingestAccepted = i.status === 202;

  if (validatorSaysOk !== ingestAccepted) {
    disagreed++;
    console.error(
      `✗ ${name}: validate says ${validatorSaysOk ? 'OK' : 'NOT OK'} ` +
        `(${v.status}) but ingest answered ${i.status}`,
    );
    console.error(`    body: ${JSON.stringify(body).slice(0, 120)}`);
  }
}

if (disagreed > 0) {
  console.error(
    `\n  ${disagreed} of ${CASES.length} bodies got different answers. A ` +
      `validator that disagrees with ingest teaches people the wrong shape.`,
  );
  process.exit(1);
}
console.log(`✓ validate and ingest agree on all ${CASES.length} bodies`);
