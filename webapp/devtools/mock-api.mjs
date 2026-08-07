// Canned API for rendering the dashboard locally. Never runs in production.
//
// KEEP THE DIRT IN. Every timestamp here was a valid ISO string on the
// first pass, which is exactly why the screenshots looked fine while
// production was throwing RangeError on twelve pages: the server was
// sending `[1970,1,0,...]` and null, and this mock never was. A mock
// that only produces clean data verifies the happy path and nothing
// else. Nullable columns should be null, lists should have rows, and at
// least one timestamp should arrive in the wrong shape.
//
// Shapes are copied from `webapp/src/lib/api.ts` — when a page renders
// blank, check that first: a wrong shape here looks exactly like a bug
// in the page.
import { createServer } from 'node:http';

const now = Date.now();
const iso = ms => new Date(now - ms).toISOString();
const PROJ = '019e358a-adac-7881-9f7e-fc92646fae4e';
const ISSUE = '019f85ee-ae41-77f1-bbf9-97d310663c9a';
const EVENT = '019f8600-0000-7000-8000-000000000001';
const U1 = '019e3589-9d7f-7013-9952-e3f287104954';
const U3 = '019e3589-0000-7000-8000-0000000000c3';
const U2 = '019f802f-c10f-7572-9cfa-f9c143d6534c';

// ── crash thread ──────────────────────────────────────────
const issue = {
  id: ISSUE, project_id: PROJ, fingerprint: 'a3f1c92e7b40d5586e1c2f8a94b7d013',
  error_type: 'TypeError', message_sample: "Cannot read property 'id' of undefined",
  culprit: 'CheckoutScreen.onPay', kind: 'error', status: 'active', level: 'error',
  event_count: 1247, user_count: 312,
  first_seen: iso(86_400_000 * 3), last_seen: iso(240_000),
  last_release: 'myapp@1.2.3', last_environment: 'production',
  assignee_id: null, release: 'myapp@1.2.3',
  // time's default Serialize — the exact shape that broke v1.7.15.
  regressed_at: [1970, 1, 0, 0, 0, 0, 0, 0, 0],
  regressed_in_release: null, resolved_at: null,
  priority: 'p1', labels: ['checkout', 'regression'], assignee_user_id: U1,
};
const issues = [
  issue,
  { ...issue, id: 'b'.repeat(8), error_type: 'NetworkError', message_sample: 'Request timed out after 30000ms', culprit: 'api.fetchCart', event_count: 88, user_count: 41, last_seen: iso(900_000), regressed_at: null, priority: 'p0', assignee_user_id: null },
  { ...issue, id: 'c'.repeat(8), error_type: 'RangeError', message_sample: 'Maximum call stack size exceeded', culprit: 'renderTree', event_count: 9, user_count: 3, status: 'resolved', last_seen: iso(7_200_000), resolved_at: iso(3_600_000), regressed_at: null, priority: 'p3', labels: [], assignee_user_id: null },
];

const t0 = 240_000;
const eventDetail = {
  id: EVENT, issue_id: ISSUE, kind: 'error', timestamp: iso(t0), received_at: iso(t0 - 1000),
  release: 'myapp@1.2.3', environment: 'production', platform: 'react-native',
  payload: {
    level: 'error',
    error: {
      type: 'TypeError', message: "Cannot read property 'id' of undefined",
      stack: [
        { file: 'node_modules/react-native/Libraries/Renderer/ReactNativeRenderer.js', line: 8213, function: 'commitRoot', inApp: false },
        { file: 'node_modules/react-native/Libraries/Renderer/ReactNativeRenderer.js', line: 7791, function: 'performSyncWork', inApp: false },
        { file: 'src/screens/CheckoutScreen.tsx', line: 142, column: 27, function: 'onPay', inApp: true,
          preContext: ['  const submit = async () => {', '    setBusy(true)', '    const cart = useCart()'],
          postContext: ['    await charge(cart.user.id)', '    setBusy(false)', '  }'] },
        { file: 'src/lib/checkout.ts', line: 58, column: 11, function: 'charge', inApp: true,
          preContext: ['export async function charge(userId: string) {', '  const token = await mintToken(userId)'],
          postContext: ['  return post(`/pay`, { token })', '}'] },
      ],
      cause: { type: 'NetworkError', message: 'mintToken timed out after 30000ms',
        stack: [{ file: 'src/lib/net.ts', line: 22, function: 'withTimeout', inApp: true }] },
    },
    breadcrumbs: [
      { type: 'nav',  timestamp: iso(t0 + 46_000), data: { from: 'Home', to: 'Cart' } },
      { type: 'user', timestamp: iso(t0 + 38_000), data: { action: 'tap', target: 'Checkout' } },
      { type: 'net',  timestamp: iso(t0 + 31_000), data: { method: 'GET', url: '/v1/cart', status: 200 } },
      { type: 'nav',  timestamp: iso(t0 + 24_000), data: { from: 'Cart', to: 'Checkout' } },
      { type: 'user', timestamp: iso(t0 + 11_000), data: { action: 'tap', target: 'Pay now' } },
      { type: 'net',  timestamp: iso(t0 + 9_000),  data: { method: 'POST', url: '/v1/pay/token', status: 504 } },
      { type: 'log',  timestamp: iso(t0 + 4_200),  data: { level: 'warn', message: 'retrying mintToken (1/3)' } },
      { type: 'log',  timestamp: iso(t0 + 600),    data: { level: 'error', message: 'mintToken gave up' } },
    ],
    device: { os: 'iOS', osVersion: '18.2', model: 'iPhone 16 Pro', locale: 'ja-JP', networkType: 'wifi' },
    app: { version: '1.2.3', build: '456', framework: { name: 'react-native', version: '0.79.2' } },
    user: { id: 'usr_8812', anonymous: false },
    tags: { screen: 'Checkout', experiment: 'pay-v2' },
  },
  attachments: [{ ref: 'replay-demo', kind: 'replay', sizeBytes: 40_960, mediaType: 'application/x-ndjson' }],
};

const events = [
  { id: EVENT, issue_id: ISSUE, kind: 'error', timestamp: iso(t0), release: 'myapp@1.2.3', environment: 'production', platform: 'ios', error_type: 'TypeError', message: "Cannot read property 'id' of undefined" },
  { id: 'e2', issue_id: 'b'.repeat(8), kind: 'error', timestamp: iso(900_000), release: 'myapp@1.2.3', environment: 'production', platform: 'android', error_type: 'NetworkError', message: 'Request timed out' },
  { id: 'e3', issue_id: ISSUE, kind: 'message', timestamp: iso(1_800_000), release: 'myapp@1.2.2', environment: 'staging', platform: 'ios', error_type: '', message: 'checkout started' },
];

// A wireframe recording: keyframe + deltas, the shape the SDK uploads.
function replayNdjson() {
  const base = now - t0 - 46_000;
  const bar = { x: 0, y: 0, w: 390, h: 88, kind: 'header', color: '#1f2937' };
  const list = i => ({ x: 16, y: 104 + i * 72, w: 358, h: 60, kind: 'row', color: '#111827' });
  const cta = { x: 16, y: 760, w: 358, h: 56, kind: 'button', text: 'Pay now', color: '#2b75ee' };
  const spin = { x: 180, y: 772, w: 32, h: 32, kind: 'spinner', color: '#93c5fd' };
  const lines = [JSON.stringify({ ts: base, kind: 'key', width: 390, height: 844, nodes: [bar, list(0), list(1), list(2), cta] })];
  for (let i = 1; i <= 34; i++) {
    const ts = base + i * 1300;
    if (i === 20) lines.push(JSON.stringify({ ts, kind: 'delta', added: [spin], changed: [], removed: [] }));
    else if (i === 33) lines.push(JSON.stringify({ ts, kind: 'delta', added: [], changed: [{ ...cta, color: '#7f1d1d', text: 'Payment failed' }], removed: [spin] }));
    else lines.push(JSON.stringify({ ts, kind: 'delta', added: [], removed: [], changed: [{ ...list(i % 3), color: i % 2 ? '#111827' : '#161f2e' }] }));
  }
  return lines.join('\n');
}

const spans = [
  { id: 's1', parent_span_id: null, op: 'http.server', name: 'POST /v1/pay', status: 'ok', started_at: iso(t0 + 9_000), duration_ms: 30_412, tags: {} },
  { id: 's2', parent_span_id: 's1', op: 'db.query', name: 'SELECT cart', status: 'ok', started_at: iso(t0 + 8_900), duration_ms: 12, tags: {} },
  { id: 's3', parent_span_id: 's1', op: 'http.client', name: 'POST payments.example', status: 'error', started_at: iso(t0 + 8_800), duration_ms: 30_000, tags: {} },
];
const trace = { trace_id: '4bf92f3577b34da6a3ce929d0e0e4736', root_op: 'http.server', root_name: 'POST /v1/pay', first_seen: iso(t0 + 9_000), last_seen: iso(t0), span_count: 3, status: 'error', duration_ms: 30_412 };

// ── everything else, one entry per GET the dashboard makes ──
const EXACT = {
  '/healthz': { status: 'ok', db: 'ok', version: '1.7.18', pool_size: 10, pool_idle: 8, push_queued: 0, push_failed_24h: 0 },
  '/auth/me': { user_id: U1, email: 'takagi@golia.jp', email_verified: true, created_at: iso(86_400_000 * 90), workspace_id: 'w1', workspace_name: 'GOLIA K.K.', role: 'owner', is_saasadmin: true },
  '/auth/oauth/providers': { github: true, google: true },
  '/auth/sessions': { sessions: [
    { id_hash_hex: 'a1b2c3d4', created_at: iso(86_400_000 * 2), last_used_at: iso(600_000), expires_at: iso(-86_400_000 * 5), ip: '203.0.113.7', user_agent: 'Mozilla/5.0 (Macintosh)' },
    { id_hash_hex: 'e5f6a7b8', created_at: iso(86_400_000 * 20), last_used_at: null, expires_at: iso(-86_400_000), ip: null, user_agent: null },
  ] },
  '/auth/notifications': { notifications: [
    { id: 'n1', kind: 'issue_new', payload: { title: 'TypeError in CheckoutScreen' }, read_at: null, created_at: iso(300_000) },
    { id: 'n2', kind: 'quota', payload: { title: 'Events at 80% of plan' }, read_at: iso(86_400_000), created_at: iso(86_400_000 * 2) },
  ], unread: 1 },
  '/v1/projects': [{ id: PROJ, slug: 'insight-mobile', name: 'insight-mobile', created_at: iso(86_400_000 * 60) }],
  '/v1/usage': { plan: 'enterprise', status: 'active', period_yyyymm: '2026-07',
    events: { count: 41_233, dropped: 0, limit: 1_000_000 },
    spans: { count: 12_004, dropped: 0, limit: 500_000 },
    replays: { count: 318, dropped: 2, limit: 5_000 } },
  // Deliberately a lapsed subscription: the downgrade banner and the
  // gap between the plan bought and the plan enforced only render in
  // this state, and a mock that always shows a healthy account never
  // exercises them.
  '/admin/api/billing': { plan: 'pro', status: 'canceled', effective_plan: 'free',
    current_period_end: iso(-86_400_000 * 12), period_yyyymm: '2026-07',
    stripe_enabled: true, webhook_configured: true,
    has_customer: true, upgradeable: { pro: true, enterprise: false },
    usage: { events: { count: 41_233, dropped: 0, limit: 1_000_000 },
             spans: { count: 12_004, dropped: 0, limit: 500_000 },
             replays: { count: 318, dropped: 2, limit: 5_000 } } },
  '/admin/api/projects/019e358a-adac-7881-9f7e-fc92646fae4e/visibility': { user_ids: [U3] },
  '/admin/api/workspaces': { workspaces: [{ workspace_id: 'w1', name: 'GOLIA K.K.', role: 'owner', active: true }] },
  '/admin/api/members': { members: [
    { user_id: U1, email: 'takagi@golia.jp', email_verified: true, role: 'owner', added_by: null, added_by_email: null, added_at: iso(86_400_000 * 40) },
    { user_id: U2, email: 'lihao@golia.jp', email_verified: false, role: 'admin', added_by: U1, added_by_email: 'takagi@golia.jp', added_at: iso(86_400_000 * 3) },
    // A `user`-role member, so the per-project access control renders.
    // Without one the column shows only the "all projects" case and the
    // panel is unreachable.
    { user_id: U3, email: 'contractor@example.com', email_verified: true, role: 'user', added_by: U1, added_by_email: 'takagi@golia.jp', added_at: iso(86_400_000) },
  ] },
  '/admin/api/invites': { invites: [
    { id: 'i1', email: 'newhire@golia.jp', role: 'user', expires_at: iso(-86_400_000 * 6), accepted_at: null, created_at: iso(86_400_000) },
  ] },
  '/admin/api/saas/stats': { workspaces: 12, active_workspaces: 9, projects: 27, users: 41, events_24h: 88_412, tokens_active: 33 },
  '/admin/api/saas/workspaces': { workspaces: [
    { id: 'w1', name: 'GOLIA K.K.', plan: 'enterprise', status: 'active', project_count: 3, member_count: 2, created_at: iso(86_400_000 * 60) },
    { id: 'w2', name: 'Acme Inc', plan: 'free', status: 'suspended', project_count: 1, member_count: 1, created_at: iso(86_400_000 * 9) },
  ] },
  '/v1/alerts': [
    { id: 'a1', project_id: PROJ, name: 'New crash in production', enabled: true, muted: false, trigger_kind: 'new_issue', trigger_config: {}, filter_config: {}, channels: { slack: '#alerts' }, throttle_minutes: 15, last_fired_at: iso(3_600_000), snoozed_until: null, created_at: iso(86_400_000 * 12) },
    { id: 'a2', project_id: null, name: 'Crash-free rate drop', enabled: false, muted: true, trigger_kind: 'crash_free_drop', trigger_config: {}, filter_config: {}, channels: {}, throttle_minutes: 60, last_fired_at: null, snoozed_until: null, created_at: iso(86_400_000 * 30) },
  ],
  '/v1/saved-views': [
    { id: 'v1', project_id: PROJ, target: 'issues', scope: 'workspace', name: 'Unresolved, production', payload: {}, created_at: iso(86_400_000 * 5) },
  ],
};

const SUFFIX = [
  [/\/attachments\//, () => null], // handled before JSON
  [/\/events\/trend/, () => Array.from({ length: 14 }, (_, i) => ({ day: new Date(now - 86_400_000 * (13 - i)).toISOString().slice(0, 10), count: 40 + ((i * 13) % 90) }))],
  [/\/events\/[^/]+$/, () => eventDetail],
  [/\/events$/, () => events],
  [/\/issues\/[^/]+\/comments$/, () => ({ comments: [{ id: 'c1', author_user_id: U1, body: 'Reproduced on 18.2 only.', created_at: iso(1_200_000), edited_at: null }] })],
  [/\/issues\/[^/]+\/watchers$/, () => ({ watchers: [{ user_id: U1, started_at: iso(1_800_000) }] })],
  [/\/issues\/[^/]+\/activity$/, () => ({ activity: [{ id: 'g1', actor_user_id: U1, kind: 'status', payload: { to: 'active' }, created_at: iso(2_400_000) }] })],
  [/\/issues\/[^/]+$/, () => issue],
  [/\/issues$/, () => issues],
  [/\/traces\/[^/]+$/, () => ({ trace, spans })],
  [/\/traces/, () => ({ traces: [trace] })],
  [/\/metrics\/[^/]+\/timeseries/, () => ({ name: 'checkout.duration', hours: 24, points: Array.from({ length: 24 }, (_, i) => ({ bucket: iso(3_600_000 * (23 - i)), sum: 1000 + i * 37, count: 10 + i, min: 12, max: 480 })) })],
  [/\/metrics$/, () => ({ metrics: [{ name: 'checkout.duration', last_bucket: iso(600_000), total_count: 8_412, avg_value: 218.4 }, { name: 'cart.size', last_bucket: null, total_count: 0, avg_value: 0 }] })],
  [/\/replays\/[^/]+$/, () => ({ replay: { id: 'r1', event_id: EVENT, blob_hash: 'deadbeef', started_at: iso(t0 + 46_000), ended_at: iso(t0), duration_ms: 44_200, frame_count: 35, created_at: iso(t0) } })],
  [/\/replays/, () => ({ replays: [{ id: 'r1', event_id: EVENT, blob_hash: 'deadbeef', started_at: iso(t0 + 46_000), ended_at: iso(t0), duration_ms: 44_200, frame_count: 35, created_at: iso(t0) }] })],
  [/\/runtime-metrics\/series/, () => ({ name: 'runtime.fps.p95', hours: 24, points:
    Array.from({ length: 24 }, (_, i) => ({ bucket_ts: iso(3_600_000 * (23 - i)), release: 'myapp@1.2.3', environment: 'production', count: 60, avg: 58, p50: 60, p95: 59 - (i % 4), p99: 51 })) })],
  [/\/runtime-metrics/, () => ({ hours: 24, metrics: [
    { name: 'runtime.cold_start_ms', bucket_ts: iso(3_600_000), release: 'myapp@1.2.3', environment: 'production', count: 1, avg: 2539, p50: 2539, p95: 2539, p99: 2539 },
    { name: 'runtime.fps.p95', bucket_ts: iso(3_600_000), release: 'myapp@1.2.3', environment: 'production', count: 66, avg: 58.4, p50: 60, p95: 59, p99: 51 },
    { name: 'runtime.heap.used_bytes', bucket_ts: iso(3_600_000), release: 'myapp@1.2.3', environment: 'production', count: 12, avg: 2.0e8, p50: 2.0e8, p95: 203098264, p99: 2.1e8 },
    { name: 'runtime.route_nav_ms', bucket_ts: iso(7_200_000), release: 'myapp@1.2.3', environment: 'production', count: 9, avg: 180, p50: 160, p95: 410, p99: 480 },
  ] })],
  [/\/user-reports/, () => ({ reports: [
    { id: 'ur1', event_id: EVENT, issue_id: ISSUE, title: 'Payment button did nothing',
      body: 'Tapped Pay now three times, the spinner ran and then it went back to the cart. Card was never charged.',
      email: 'customer@example.com', name: 'Aiko', received_at: iso(600_000) },
  ] })],
  [/\/track\/names/, () => ({ days: 7, names: [
    { name: '$pageview', total: 18_751, users: 45, last_seen: iso(3_600_000) },
    { name: 'bio.login.attempt', total: 177, users: 0, last_seen: iso(7_200_000) },
    { name: 'bio.login.failure', total: 148, users: 0, last_seen: iso(9_000_000) },
    { name: 'checkout.started', total: 62, users: 31, last_seen: iso(600_000) },
  ] })],
  [/\/track\/series/, () => ({ name: '$pageview', days: 30, points:
    Array.from({ length: 30 }, (_, i) => ({ day: iso(86_400_000 * (29 - i)), total: 400 + ((i * 37) % 300), users: 3 + (i % 9) })) })],
  [/\/track\/recent/, () => ({ events: [
    { id: 't1', name: '$pageview', user_id: 'a91f3c02deadbeef', session_id: null, route: '/checkout', release: 'myapp@1.2.3', environment: 'production', props: { referrer: '/cart' }, occurred_at: iso(120_000) },
    { id: 't2', name: 'checkout.started', user_id: 'a91f3c02deadbeef', session_id: null, route: '/checkout', release: 'myapp@1.2.3', environment: 'production', props: { items: 3 }, occurred_at: iso(180_000) },
    { id: 't3', name: 'bio.login.failure', user_id: null, session_id: null, route: '/login', release: 'myapp@1.2.3', environment: 'production', props: {}, occurred_at: iso(900_000) },
  ] })],
  [/\/stats$/, () => ({ events_24h: 1382, issues_active: 2, spans_24h: 940, metrics_buckets_24h: 288, replays_24h: 6 })],
  [/\/search/, () => ({ q: '', issues: [{ id: ISSUE, error_type: 'TypeError', message_sample: "Cannot read property 'id' of undefined", last_seen: iso(240_000) }], events: [{ id: EVENT, issue_id: ISSUE, kind: 'error', timestamp: iso(240_000), release: 'myapp@1.2.3' }] })],
  [/\/endpoint-probes$/, () => ({ probes: [{ id: 'p1', endpoint_url: 'https://api.example.com/health', method: 'GET', interval_seconds: 60, enabled: true, last_status: 200, last_checked_at: iso(45_000), created_at: iso(86_400_000 * 4) }] })],
  [/\/cert\/observations$/, () => ([{ id: 'o1', project_id: PROJ, domain: 'api.example.com', common_name: 'api.example.com', issuer_name: "Let's Encrypt R11", not_before: iso(86_400_000 * 20), not_after: iso(-86_400_000 * 70), observed_at: iso(3_600_000) }])],
  [/\/cert\/watches$/, () => ([{ id: 'w1', project_id: PROJ, domain: 'api.example.com', added_by: U1, added_at: iso(86_400_000 * 20) }])],
  [/\/audit/, () => [{ id: 'au1', project_id: PROJ, actor_user_id: U1, action: 'token.mint', target_type: 'token', target_id: 'tk1', payload: {}, created_at: iso(7_200_000) }]],
  [/\/tokens$/, () => ({ tokens: [
    { id: 'tk1', kind: 'public', label: 'production iOS', last4: 'a91f', created_at: iso(86_400_000 * 30), revoked_at: null },
    { id: 'tk2', kind: 'public', label: null, last4: '0c3d', created_at: iso(86_400_000 * 55), revoked_at: iso(86_400_000 * 2) },
  ] })],
  [/\/push\/credentials$/, () => ({ credentials: [{ id: 'pc1', kind: 'apns', config: {}, created_at: iso(86_400_000 * 15), last_validated_at: iso(86_400_000), last_validate_status: 'ok' }] })],
  [/\/push\/sends/, () => ({ sends: [{ id: 'ps1', token_id: 'dt1', provider: 'apns', status: 'failed', attempts: 3, created_at: iso(1_800_000), sent_at: null, next_attempt_at: iso(-600_000), error: 'BadDeviceToken' }] })],
  [/\/integrations$/, () => ({ integrations: [{ id: 'in1', kind: 'slack', config: { channel: '#alerts' }, connected_by: U1, connected_at: iso(86_400_000 * 8), active: true }] })],
  [/\/releases\/[^/]+\/artifacts$/, () => ({ artifacts: [{ id: 'ar1', kind: 'sourcemap', name: 'index.android.bundle.map', content_hash: 'abc123', size_bytes: 4_194_304, created_at: iso(86_400_000) }] })],
  [/\/releases$/, () => ({ releases: [{ id: 'rl1', name: 'myapp@1.2.3+456', created_at: iso(86_400_000), deploy_at: iso(86_400_000) }, { id: 'rl2', name: 'myapp@1.2.2+441', created_at: iso(86_400_000 * 9), deploy_at: null }] })],
  [/\/projects\/[^/]+$/, () => ({ id: PROJ, slug: 'insight-mobile', name: 'insight-mobile', created_at: iso(86_400_000 * 60) })],
];

createServer((req, res) => {
  const p = new URL(req.url, 'http://x').pathname;
  // The replay page fetches by replay id; the crash view fetches the
  // same bytes by attachment ref. Both are NDJSON, not JSON.
  if (p.includes('/attachments/') || p.endsWith('/ndjson')) {
    res.setHeader('content-type', 'application/x-ndjson');
    res.end(replayNdjson());
    return;
  }
  res.setHeader('content-type', 'application/json');
  if (p in EXACT) return res.end(JSON.stringify(EXACT[p]));
  for (const [re, make] of SUFFIX) {
    if (re.test(p)) return res.end(JSON.stringify(make()));
  }
  process.stdout.write(`UNMOCKED ${p}\n`);
  res.end('{}');
}).listen(8080, () => console.log('mock api :8080'));
