// Generate the OpenAPI document from the router, and check it in CI.
//
//   node scripts/gen-openapi.mjs            # write
//   node scripts/gen-openapi.mjs --check    # fail if stale
//
// A clean-context agent asked for this by name: field lists, required
// vs optional, and defaults lived in prose and markdown tables, and
// the only copy it trusted was a `.d.ts` whose CDN path it had to
// guess. Prose is where documentation and server drift apart; this
// file is written from `handlers/mod.rs`, so the path list cannot.
//
// The schemas are hand-written — Rust types are not machine-read here
// — but check-openapi-schema.mjs holds the request body to the fields
// serde actually accepts, so that half cannot drift silently either.
import { readFileSync, writeFileSync } from 'node:fs';

const ROOT = new URL('..', import.meta.url).pathname;
const MOD = 'self-hosted/server/src/handlers/mod.rs';
const OUT = 'self-hosted/server/openapi.json';

const src = readFileSync(ROOT + MOD, 'utf8');
const routes = [];
for (const m of src.matchAll(/\.route\(\s*"([^"]+)"\s*,/g)) {
  const tail = src.slice(m.index + m[0].length, m.index + m[0].length + 400);
  const next = tail.indexOf('.route(');
  const seg = next === -1 ? tail : tail.slice(0, next);
  const methods = [...new Set(
    [...seg.matchAll(/\b(get|post|put|patch|delete)\s*\(/g)].map((x) => x[1]),
  )];
  routes.push({ path: m[1], methods });
}
if (routes.length < 50) {
  console.error(`✗ parsed ${routes.length} routes from ${MOD}. Broken checker.`);
  process.exit(1);
}

// Only the machine-facing surface belongs in a document an integrator
// reads: /v1 is the SDK wire, /api is the token-scoped read path.
// /admin is the console's own API and is not a contract with anyone.
const PUBLIC = routes
  .filter((r) => r.path.startsWith('/v1') || r.path.startsWith('/api/')
    || ['/healthz', '/livez', '/readyz', '/openapi.json'].includes(r.path))
  .sort((a, b) => a.path.localeCompare(b.path));

const VERSION = readFileSync(ROOT + 'VERSION', 'utf8').trim();

const ERROR = {
  type: 'object',
  properties: {
    error: { type: 'string', description: 'Stable code. Branch on this.' },
    detail: { type: 'string', description: 'Prose. Changes without notice.' },
    hint: { type: 'string', description: 'Prose. Changes without notice.' },
  },
  required: ['error'],
};

const WIRE_EVENT = {
  type: 'object',
  required: ['kind', 'occurredAt', 'platform'],
  properties: {
    kind: { type: 'string', enum: ['error', 'warn', 'trace', 'assert', 'probe'] },
    occurredAt: {
      type: 'string', format: 'date-time',
      description: 'RFC 3339. No alias — `timestamp` is dropped as unknown and the request then fails as a missing field.',
    },
    platform: { type: 'string', enum: ['javascript', 'ios', 'android'] },
    release: { type: 'string', default: '' },
    environment: { type: 'string', default: '' },
    userKey: { type: 'string', nullable: true },
    name: { type: 'string', nullable: true },
    surface: { nullable: true },
    id: { type: 'string', format: 'uuid', description: 'Optional; supply one only to deduplicate your own retries.' },
    payload: {
      type: 'object', default: {},
      description: 'Everything else about the event — error, device, app, and any data you attach.',
    },
  },
};

const RECEIPT = {
  type: 'object',
  required: ['eventId', 'issueId', 'isNewIssue', 'regressed'],
  properties: {
    eventId: { type: 'string', format: 'uuid' },
    issueId: { type: 'string', format: 'uuid', description: 'The machine-readable receipt: assert on this rather than opening the dashboard.' },
    isNewIssue: { type: 'boolean' },
    regressed: { type: 'boolean' },
  },
};

const unauthorized = {
  description: 'Missing, malformed, unknown, or wrong-scope token. `hint` names which.',
  content: { 'application/json': { schema: { $ref: '#/components/schemas/Error' } } },
};

// Paths that carry a described contract. Everything else in PUBLIC is
// listed with its methods so the document is complete, but without a
// body schema — better an honest stub than an invented one.
const DESCRIBED = {
  '/v1/events': {
    post: {
      summary: 'Ingest one event',
      security: [{ bearer: [] }],
      requestBody: { required: true, content: { 'application/json': { schema: { $ref: '#/components/schemas/WireEvent' } } } },
      responses: {
        202: { description: 'Accepted', content: { 'application/json': { schema: { $ref: '#/components/schemas/Receipt' } } } },
        400: { description: '`invalid_payload` — parsed, but a value is not acceptable.', content: { 'application/json': { schema: { $ref: '#/components/schemas/Error' } } } },
        401: unauthorized,
        422: { description: 'Deserialisation failed. **Body is plain text**, not this error shape — it names the field, e.g. `missing field `occurredAt``.', content: { 'text/plain': { schema: { type: 'string' } } } },
        429: { description: '`rate_limited`, with `retryAfterMs`.', content: { 'application/json': { schema: { $ref: '#/components/schemas/Error' } } } },
      },
    },
  },
  '/v1/events:batch': {
    post: {
      summary: 'Ingest up to 200 events',
      security: [{ bearer: [] }],
      requestBody: { required: true, content: { 'application/json': { schema: { type: 'object', required: ['events'], properties: { events: { type: 'array', maxItems: 200, items: { $ref: '#/components/schemas/WireEvent' } } } } } } },
      responses: {
        202: { description: 'Accepted' },
        401: unauthorized,
        413: { description: '`batch_too_large` — more than 200 events.', content: { 'application/json': { schema: { $ref: '#/components/schemas/Error' } } } },
      },
    },
  },
  '/v1/events/validate': {
    post: {
      summary: 'Does this body parse? No token required.',
      description: 'Parses exactly what ingest parses and answers whether it would have been accepted. Touches no database, names no project, stores nothing, and says nothing about whether a token would work. Rate-limited per IP.',
      requestBody: { required: true, content: { 'application/json': { schema: { $ref: '#/components/schemas/WireEvent' } } } },
      responses: {
        200: { description: 'Parses. `parsed` echoes what was understood, so a field silently dropped as unknown is visible.', content: { 'application/json': { schema: { type: 'object', properties: { ok: { type: 'boolean' }, parsed: { type: 'object' }, note: { type: 'string' } } } } } },
        400: { description: 'Does not parse. `field` names the offending key where serde identified one.', content: { 'application/json': { schema: { type: 'object', properties: { ok: { type: 'boolean' }, error: { type: 'string' }, detail: { type: 'string' }, field: { type: 'string', nullable: true }, hint: { type: 'string' } } } } } },
        429: { description: '`rate_limited`.' },
      },
    },
  },
  '/healthz': {
    get: {
      summary: 'Liveness and version. No auth.',
      responses: { 200: { description: 'ok', content: { 'application/json': { schema: { type: 'object', properties: { status: { type: 'string' }, db: { type: 'string' }, version: { type: 'string' } } } } } } },
    },
  },
};

const paths = {};
for (const r of PUBLIC) {
  paths[r.path] = {};
  for (const method of r.methods) {
    paths[r.path][method] = DESCRIBED[r.path]?.[method] ?? {
      summary: `${method.toUpperCase()} ${r.path}`,
      description: 'Listed for completeness; request and response shapes are in docs/protocol.md.',
      ...(r.path.startsWith('/healthz') || r.path.startsWith('/livez') || r.path.startsWith('/readyz')
        ? {}
        : { security: [{ bearer: [] }] }),
      responses: { default: { description: 'See docs/protocol.md' } },
    };
  }
}
// Described paths the router does not serve would be a lie.
for (const p of Object.keys(DESCRIBED)) {
  if (!paths[p]) {
    console.error(`✗ ${OUT} would describe ${p}, which ${MOD} does not route.`);
    process.exit(1);
  }
}

const doc = {
  openapi: '3.1.0',
  info: {
    title: 'Sentori',
    version: VERSION,
    summary: 'Self-hosted crash and error reporting for mobile apps.',
    description:
      'Its own wire protocol — not Sentry-compatible, and there is no DSN. ' +
      'Generated from the router by scripts/gen-openapi.mjs; the path list ' +
      'cannot drift from the server.',
  },
  servers: [{ url: '{instance}', variables: { instance: { default: 'https://sentori.example.com' } } }],
  components: {
    securitySchemes: {
      bearer: { type: 'http', scheme: 'bearer', description: 'A token from Settings ▸ Tokens: `st_` plus 26 characters. Scope `ingest` writes; scope `api` reads.' },
    },
    schemas: { WireEvent: WIRE_EVENT, Receipt: RECEIPT, Error: ERROR },
  },
  paths,
};

const body = JSON.stringify(doc, null, 2) + '\n';
if (process.argv.includes('--check')) {
  let existing = '';
  try { existing = readFileSync(ROOT + OUT, 'utf8'); } catch {
    console.error(`✗ ${OUT} is missing. Run: node scripts/gen-openapi.mjs`);
    process.exit(1);
  }
  if (existing !== body) {
    // `info.version` tracks VERSION, so a release bump makes this
    // stale too — that is the point, and it is the most common reason
    // to see this message.
    console.error(
      `✗ ${OUT} is stale — the router or VERSION changed and it did not.` +
        `\n  Run: node scripts/gen-openapi.mjs`,
    );
    process.exit(1);
  }
  console.log(`✓ ${OUT} matches the router (${PUBLIC.length} paths)`);
} else {
  writeFileSync(ROOT + OUT, body);
  console.log(`✓ wrote ${OUT} — ${PUBLIC.length} paths, ${Object.keys(DESCRIBED).length} with full schemas`);
}
