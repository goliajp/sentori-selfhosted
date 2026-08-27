// Many devices reporting the same new fault at once must not 500.
//
//   SENTORI_BASE=… SENTORI_TOKEN=… node scripts/check-ingest-concurrency.mjs
//
// An issue row is created exactly once per fingerprint, and the moment
// it is created is the moment a new fault first fires — across every
// device running the broken build, simultaneously. That is the worst
// possible time to be racing, and it was: `SELECT … FOR UPDATE` locks
// rows that exist and cannot stop a concurrent insert of one that does
// not, so both callers inserted and one died on the unique constraint.
// Nine of two hundred, as a 500 the SDK is contractually told to retry.
//
// This drives the shape that produced it: one fingerprint nobody has
// seen, many senders, all at once.
const BASE = process.env.SENTORI_BASE;
const TOKEN = process.env.SENTORI_TOKEN;
if (!BASE || !TOKEN) {
  console.error('set SENTORI_BASE and SENTORI_TOKEN');
  process.exit(2);
}

const N = Number(process.env.CONCURRENCY ?? 60);
const send = (name) =>
  fetch(`${BASE}/v1/events`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      authorization: `Bearer ${TOKEN}`,
    },
    body: JSON.stringify({
      kind: 'trace',
      occurredAt: new Date().toISOString(),
      platform: 'ios',
      name,
    }),
  }).then((r) => r.status);

// A fingerprint this instance has never seen, so every sender races to
// create the issue rather than to update it.
const fresh = `concurrency.probe.${process.hrtime.bigint()}`;
const codes = await Promise.all(Array.from({ length: N }, () => send(fresh)));

const tally = codes.reduce((m, c) => m.set(c, (m.get(c) ?? 0) + 1), new Map());
const fives = codes.filter((c) => c >= 500).length;
const accepted = tally.get(202) ?? 0;

// 429 is a correct answer under load and does not count against this.
const summary = [...tally.entries()]
  .sort(([a], [b]) => a - b)
  .map(([c, n]) => `${c}×${n}`)
  .join(' ');

if (fives > 0) {
  console.error(
    `✗ ${fives} of ${N} concurrent sends of one new fingerprint answered 5xx — ${summary}`,
  );
  console.error(
    '  A 5xx here is a retry the SDK is told to make, at the exact moment ' +
      'a new fault is firing across every device at once.',
  );
  process.exit(1);
}
if (accepted === 0) {
  console.error(
    `✗ none of ${N} sends were accepted — ${summary}. This harness proved ` +
      `nothing about concurrency; it never got past the door.`,
  );
  process.exit(1);
}
console.log(`✓ ${N} concurrent sends of one new fingerprint, no 5xx — ${summary}`);
