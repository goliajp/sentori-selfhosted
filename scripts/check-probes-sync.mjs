// The CLI's probe registration endpoint, at its edges.
//
//   SENTORI_BASE=… SENTORI_API_TOKEN=… node scripts/check-probes-sync.mjs
//
// It was the last route nothing had ever exercised. It had no ceiling
// on `refs` — a scan could send a million and get a million round
// trips — and it counted only its successes, so a database error left
// the caller reading `registered: 3` for five refs with no log line
// and nothing to act on.
//
// The duplicate case is the one that matters most for the fix. A
// static scan finding the same `sentori.probe(ref)` in two files sends
// it twice; the row-at-a-time loop tolerated that by accident, and a
// single statement does not — Postgres refuses to let one command's
// ON CONFLICT DO UPDATE touch a row twice. Dedup is what makes
// batching safe, so it is asserted rather than assumed.
const BASE = process.env.SENTORI_BASE;
const TOKEN = process.env.SENTORI_API_TOKEN;
if (!BASE || !TOKEN) {
  console.error('set SENTORI_BASE and SENTORI_API_TOKEN');
  process.exit(2);
}

const sync = async (body, token = TOKEN) => {
  const r = await fetch(`${BASE}/api/probes:sync`, {
    method: 'POST',
    headers: { 'content-type': 'application/json', authorization: `Bearer ${token}` },
    body: JSON.stringify(body),
  });
  let json = null;
  try { json = await r.json(); } catch { /* not json */ }
  return { status: r.status, json };
};

const fails = [];
const expect = (name, cond, got) => {
  if (!cond) fails.push(`${name} — got ${JSON.stringify(got)}`);
};

const rel = `probe.check@${process.hrtime.bigint()}`;

const plain = await sync({ release: rel, refs: ['P-1', 'P-2', 'P-3'] });
expect('three distinct refs register', plain.status === 200 && plain.json?.registered === 3, plain);

// Same refs again: an ON CONFLICT update, not a failure.
const again = await sync({ release: rel, refs: ['P-1', 'P-2', 'P-3'] });
expect('re-registering the same refs succeeds', again.status === 200, again);

// Duplicates inside one call.
const dupes = await sync({ release: rel, refs: ['P-9', 'P-9', 'P-9'] });
expect(
  'duplicates within one call do not 500',
  dupes.status === 200 && dupes.json?.registered === 1 && dupes.json?.duplicatesIgnored === 2,
  dupes,
);

const empty = await sync({ release: rel, refs: [] });
expect('an empty scan is not an error', empty.status === 200 && empty.json?.registered === 0, empty);

const huge = await sync({
  release: rel,
  refs: Array.from({ length: 1001 }, (_, i) => `BIG-${i}`),
});
expect('over the ceiling is refused', huge.status === 413, huge);

const nope = await sync({ release: rel, refs: ['X'] }, 'st_aaaaaaaaaaaaaaaaaaaaaaaaaa');
expect('an unknown token is refused', nope.status === 401, nope);

if (fails.length > 0) {
  console.error(`✗ ${fails.length} probes:sync assertion(s) failed:`);
  for (const f of fails) console.error(`    ${f}`);
  process.exit(1);
}
console.log('✓ probes:sync holds at its edges (6 assertions)');
