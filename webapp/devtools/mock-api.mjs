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
// in the page. `UNMOCKED <path>` on stdout names anything the app asked
// for that this file does not answer; the sweep fails on those, because
// a route rendering its empty state for lack of a mock is the most
// convincing way to miss a bug.
import { createServer } from 'node:http';

const now = Date.now();
const iso = (ms) => new Date(now - ms).toISOString();
const PROJ = '019e358a-adac-7881-9f7e-fc92646fae4e';
const PROJ2 = '019e358a-bbbb-7881-9f7e-fc9264600002';
const ISSUE = '019f85ee-ae41-77f1-bbf9-97d310663c9a';
const EVENT = '019f8600-0000-7000-8000-000000000001';
const U1 = '019e3589-9d7f-7013-9952-e3f287104954';
const U2 = '019f802f-c10f-7572-9cfa-f9c143d6534c';

const t0 = 240_000;

// ── the queue ─────────────────────────────────────────────
// Five kinds, three statuses, and the awkward shapes: a bare-classname
// title that must demote behind its message, a pre-split issue whose
// environment/platform are still NULL, a resolved-then-regressed one,
// and an anonymous issue with no user count.
const baseIssue = {
  projectId: PROJ,
  kind: 'error',
  title: 'TypeError',
  messageSample: "Cannot read property 'id' of undefined",
  surface: { screen: 'Checkout', element: 'PayButton' },
  status: 'open',
  firstSeen: iso(86_400_000 * 3),
  lastSeen: iso(t0),
  eventCount: 1247,
  usersCount: 312,
  maxPerUser: 9,
  lastRelease: 'myapp@1.2.3+456',
  assigneeUserId: U1,
  resolvedAt: null,
  resolvedInRelease: null,
  regressedAt: null,
  regressedInRelease: null,
  environment: 'production',
  platform: 'ios',
};

const issues = [
  { ...baseIssue, id: ISSUE },
  {
    ...baseIssue,
    id: '019f85ee-0002-77f1-bbf9-97d310660002',
    kind: 'error',
    // Production shape: the class name is the whole title and the
    // surface is empty, so the message is the only thing that
    // distinguishes this row. Five of nine real error issues in the
    // dogfood project look exactly like this.
    title: 'Error',
    messageSample: 'pinning mismatch on identity.focusai.com (mode=report-only)',
    surface: {},
    platform: 'android',
    eventCount: 88,
    usersCount: 41,
    maxPerUser: 3,
    lastSeen: iso(900_000),
    assigneeUserId: null,
    // Resolved, then it came back: the regression chip and the
    // "reopened" activity line only render in this state.
    status: 'open',
    resolvedAt: iso(86_400_000 * 2),
    resolvedInRelease: 'myapp@1.2.2+441',
    regressedAt: iso(3_600_000),
    regressedInRelease: 'myapp@1.2.3+456',
  },
  {
    ...baseIssue,
    id: '019f85ee-0003-77f1-bbf9-97d310660003',
    kind: 'assert',
    // A bare TitleCase token: the page must lead with the message.
    title: 'Error',
    messageSample: 'cart.total matched the server',
    surface: {},
    status: 'resolved',
    eventCount: 9,
    usersCount: 3,
    maxPerUser: 1,
    lastSeen: iso(7_200_000),
    resolvedAt: iso(3_600_000),
    resolvedInRelease: 'myapp@1.2.3+456',
    assigneeUserId: null,
    environment: 'staging',
  },
  {
    ...baseIssue,
    id: '019f85ee-0004-77f1-bbf9-97d310660004',
    kind: 'probe',
    title: 'checkout-double-charge',
    messageSample: 'probe fired',
    surface: {},
    status: 'open',
    eventCount: 2,
    usersCount: 0, // anonymous: the impact line has no user half
    maxPerUser: 0,
    lastSeen: iso(1_800_000),
    assigneeUserId: null,
    platform: 'android',
  },
  {
    ...baseIssue,
    id: '019f85ee-0005-77f1-bbf9-97d310660005',
    kind: 'warn',
    title: 'slow frame budget exceeded',
    messageSample: 'render took 812ms',
    surface: { screen: 'Feed' },
    status: 'ignored',
    eventCount: 402,
    usersCount: 77,
    maxPerUser: 21,
    lastSeen: iso(600_000),
    assigneeUserId: null,
    // Pre-2.9 issue: aggregated before the env × platform split, so
    // both columns are NULL and the row must still render.
    environment: null,
    platform: null,
  },
  {
    ...baseIssue,
    id: '019f85ee-0006-77f1-bbf9-97d310660006',
    kind: 'trace',
    title: 'app.launch',
    messageSample: 'staged launch',
    surface: {},
    status: 'open',
    eventCount: 5120,
    usersCount: 980,
    maxPerUser: 41,
    lastSeen: iso(120_000),
    assigneeUserId: null,
  },
];

const activity = [
  {
    id: 'g1',
    at: iso(2_400_000),
    kind: 'status',
    body: { to: 'open' },
    actorUserId: U1,
    actorEmail: 'takagi@golia.jp',
  },
  {
    id: 'g2',
    at: iso(2_000_000),
    kind: 'assign',
    body: { to: U1 },
    actorUserId: U1,
    actorEmail: 'takagi@golia.jp',
  },
  {
    id: 'g3',
    at: iso(1_200_000),
    kind: 'note',
    body: { text: 'Reproduced on iOS 18.2 only. Android is clean.' },
    actorUserId: U2,
    actorEmail: 'lihao@golia.jp',
  },
  // A system-authored line: no actor, and the page must not print
  // "null" where the name goes.
  {
    id: 'g4',
    at: iso(3_600_000),
    kind: 'regression',
    body: { release: 'myapp@1.2.3+456' },
    actorUserId: null,
    actorEmail: null,
  },
];

const issueReleases = [
  {
    release: 'myapp@1.2.3+456',
    events: 1180,
    firstAt: iso(86_400_000),
    lastAt: iso(t0),
  },
  {
    release: 'myapp@1.2.2+441',
    events: 67,
    firstAt: iso(86_400_000 * 3),
    lastAt: iso(86_400_000 * 2),
  },
];

const occurrences = [
  {
    id: EVENT,
    kind: 'error',
    platform: 'ios',
    occurredAt: iso(t0),
    receivedAt: iso(t0 - 1200),
    release: 'myapp@1.2.3+456',
    environment: 'production',
    userKey: 'a91f3c02deadbeefa91f3c02deadbeef',
    screensRef: 'replay-demo',
  },
  {
    id: 'e2',
    kind: 'error',
    platform: 'ios',
    occurredAt: iso(900_000),
    receivedAt: iso(899_000),
    release: 'myapp@1.2.3+456',
    environment: 'production',
    userKey: null,
    screensRef: null,
  },
  {
    id: 'e3',
    kind: 'error',
    platform: 'android',
    occurredAt: iso(1_800_000),
    receivedAt: iso(1_799_000),
    release: 'myapp@1.2.2+441',
    environment: 'staging',
    userKey: 'ff20aa11ff20aa11ff20aa11ff20aa11',
    screensRef: null,
  },
];

const eventDetail = {
  id: EVENT,
  projectId: PROJ,
  issueId: ISSUE,
  kind: 'error',
  platform: 'ios',
  occurredAt: iso(t0),
  receivedAt: iso(t0 - 1200),
  release: 'myapp@1.2.3+456',
  environment: 'production',
  userKey: 'a91f3c02deadbeefa91f3c02deadbeef',
  payload: {
    error: {
      type: 'TypeError',
      message: "Cannot read property 'id' of undefined",
      stack: [
        {
          file: 'node_modules/react-native/Libraries/Renderer/ReactNativeRenderer.js',
          line: 8213,
          function: 'commitRoot',
          inApp: false,
        },
        {
          file: 'node_modules/react-native/Libraries/Renderer/ReactNativeRenderer.js',
          line: 7791,
          function: 'performSyncWork',
          inApp: false,
        },
        {
          file: 'src/screens/CheckoutScreen.tsx',
          line: 142,
          column: 27,
          function: 'onPay',
          inApp: true,
          preContext: ['  const submit = async () => {', '    setBusy(true)', '    const cart = useCart()'],
          contextLine: '    await charge(cart.user.id)',
          postContext: ['    setBusy(false)', '  }', ''],
        },
        // A frame the symbolicator could not resolve: no line, no
        // column, and a minified name. It must not look like a bug.
        { file: 'index.android.bundle', function: 'e', inApp: true },
        {
          file: 'src/lib/checkout.ts',
          line: 58,
          column: 11,
          function: 'charge',
          inApp: true,
          preContext: ['export async function charge(userId: string) {'],
          contextLine: '  const token = await mintToken(userId)',
          postContext: ['  return post(`/pay`, { token })', '}'],
        },
      ],
    },
    signals: [
      { t: -46.0, kind: 'nav', data: { from: 'Home', to: 'Cart' } },
      { t: -38.4, kind: 'tap', data: { target: 'Checkout', x: 196, y: 741 } },
      {
        t: -31.2,
        kind: 'http',
        data: { method: 'GET', url: '/v1/cart', status: 200, ms: 142 },
      },
      { t: -24.0, kind: 'nav', data: { from: 'Cart', to: 'Checkout' } },
      { t: -11.1, kind: 'tap', data: { target: 'Pay now', x: 195, y: 788 } },
      {
        t: -9.0,
        kind: 'http',
        data: { method: 'POST', url: '/v1/pay/token', status: 504, ms: 30_012 },
      },
      {
        t: -4.2,
        kind: 'log',
        data: { level: 'warn', message: 'retrying mintToken (1/3)' },
      },
      // No data at all: the row still needs a label.
      { t: -0.6, kind: 'background' },
    ],
    device: {
      os: 'iOS',
      osVersion: '18.2',
      model: 'iPhone 16 Pro',
      locale: 'ja-JP',
      network: 'wifi',
      batteryLevel: 0.14,
      lowPowerMode: true,
      screenWidth: 393,
      screenHeight: 852,
      scale: 3,
      memoryUsedMb: 412,
      storageFreeMb: 1180,
      orientation: 'portrait',
    },
    context: { tenant: 'acme', plan: 'pro', experiment: 'pay-v2' },
    // The SDK sets this when pixel replay was running and the ring
    // still came up empty. It cannot render on THIS event — a
    // `screens` attachment is present, so the pixel player wins —
    // and the rich fixture is worth more here than the empty branch.
    // Verified by removing the attachment locally; see
    // `issue.replayScreensEmpty`.
    replay: { screens: 'empty', captured: 0 },
    // Pixel replay was running and produced nothing — an older
    // native binary. The page must say that rather than tell the
    // reader to enable a setting that is already on.
    replay: { screens: 'empty', captured: 0 },
    app: { version: '1.2.3', build: '456' },
  },
  attachments: [
    {
      ref: 'wireframe-demo',
      kind: 'replay',
      mediaType: 'application/x-ndjson',
      sizeBytes: 12_288,
      capturedAt: iso(t0),
    },
    {
      ref: 'replay-demo',
      kind: 'screens',
      mediaType: 'application/x-ndjson',
      sizeBytes: 40_960,
      capturedAt: iso(t0),
    },
  ],
};

const contextEvents = [
  {
    id: 'c1',
    issueId: '019f85ee-0006-77f1-bbf9-97d310660006',
    kind: 'trace',
    name: 'app.launch',
    occurredAt: iso(t0 + 52_000),
  },
  {
    id: 'c2',
    issueId: '019f85ee-0003-77f1-bbf9-97d310660003',
    kind: 'assert',
    name: 'cart.total matched the server',
    occurredAt: iso(t0 + 18_000),
  },
  {
    id: 'c3',
    issueId: '019f85ee-0005-77f1-bbf9-97d310660005',
    kind: 'warn',
    name: 'render took 812ms',
    occurredAt: iso(t0 + 3_000),
  },
];

// A pixel recording: keyframe + deltas, the shape the SDK uploads.
// `screens` frames carry the window's logical size so the tap dots can
// be placed in the same coordinate space the SDK reported them in.
// The pixel recording the `screens` attachment carries: one encoded
// frame per line, `t` in seconds relative to the event (negative), and
// the window's logical size so tap coordinates land in the same space.
// SVG stands in for the SDK's JPEGs — it is what an <img> can render
// from a data URI without shipping binary fixtures into the repo.
function screensNdjson() {
  const W = 393;
  const H = 852;
  const frame = (i, n) => {
    const stage = i / (n - 1);
    const cta =
      stage > 0.94
        ? { fill: '#7f1d1d', label: 'Payment failed' }
        : { fill: '#2b75ee', label: 'Pay now' };
    const rows = [0, 1, 2]
      .map(
        (r) =>
          `<rect x="16" y="${104 + r * 72}" width="361" height="60" rx="10" fill="${
            (i + r) % 2 ? '#111827' : '#161f2e'
          }"/>`,
      )
      .join('');
    const spinner =
      stage > 0.55 && stage <= 0.94
        ? `<circle cx="196" cy="788" r="14" fill="none" stroke="#93c5fd" stroke-width="3" stroke-dasharray="60 28" transform="rotate(${
            i * 47
          } 196 788)"/>`
        : '';
    return (
      `<svg xmlns="http://www.w3.org/2000/svg" width="${W}" height="${H}" viewBox="0 0 ${W} ${H}">` +
      `<rect width="${W}" height="${H}" fill="#0b0f16"/>` +
      `<rect width="${W}" height="88" fill="#1f2937"/>` +
      `<text x="16" y="56" fill="#e5e7eb" font-family="sans-serif" font-size="20">Checkout</text>` +
      rows +
      `<rect x="16" y="760" width="361" height="56" rx="12" fill="${cta.fill}"/>` +
      `<text x="196" y="795" fill="#ffffff" font-family="sans-serif" font-size="17" text-anchor="middle">${cta.label}</text>` +
      spinner +
      '</svg>'
    );
  };
  const n = 24;
  const lines = [];
  for (let i = 0; i < n; i++) {
    lines.push(
      JSON.stringify({
        t: -46 + i * 2,
        mediaType: 'image/svg+xml',
        base64: Buffer.from(frame(i, n)).toString('base64'),
        w: W,
        h: H,
      }),
    );
  }
  // A truncated tail: uploads are append-only, so the last line dying
  // mid-write is the expected damage, and one bad line must not cost
  // the recording.
  lines.push('{"t":-0.4,"media');
  return lines.join('\n');
}

function wireframeNdjson() {
  const base = now - t0 - 46_000;
  const bar = { x: 0, y: 0, w: 393, h: 88, kind: 'header', color: '#1f2937' };
  const row = (i) => ({
    x: 16,
    y: 104 + i * 72,
    w: 361,
    h: 60,
    kind: 'row',
    color: '#111827',
  });
  const cta = {
    x: 16,
    y: 760,
    w: 361,
    h: 56,
    kind: 'button',
    text: 'Pay now',
    color: '#2b75ee',
  };
  const spin = {
    x: 180,
    y: 772,
    w: 32,
    h: 32,
    kind: 'spinner',
    color: '#93c5fd',
  };
  const lines = [
    JSON.stringify({
      ts: base,
      kind: 'key',
      width: 393,
      height: 852,
      w: 393,
      h: 852,
      nodes: [bar, row(0), row(1), row(2), cta],
    }),
  ];
  for (let i = 1; i <= 34; i++) {
    const ts = base + i * 1300;
    if (i === 20)
      lines.push(
        JSON.stringify({
          ts,
          kind: 'delta',
          added: [spin],
          changed: [],
          removed: [],
        }),
      );
    else if (i === 33)
      lines.push(
        JSON.stringify({
          ts,
          kind: 'delta',
          added: [],
          changed: [{ ...cta, color: '#7f1d1d', text: 'Payment failed' }],
          removed: [spin],
        }),
      );
    else
      lines.push(
        JSON.stringify({
          ts,
          kind: 'delta',
          added: [],
          removed: [],
          changed: [{ ...row(i % 3), color: i % 2 ? '#111827' : '#161f2e' }],
        }),
      );
  }
  // A truncated tail — append-only uploads end mid-line when the
  // process dies, and one bad line must not cost the recording.
  lines.push('{"ts":');
  return lines.join('\n');
}

// ── everything else, one entry per GET the dashboard makes ──
const EXACT = {
  '/healthz': {
    status: 'ok',
    db: 'ok',
    version: '2.10.0',
    pool_size: 4,
    pool_idle: 3,
    push_queued: 0,
    push_failed_24h: 0,
  },
  '/auth/me': {
    userId: U1,
    email: 'takagi@golia.jp',
    displayName: null,
    role: 'superadmin',
  },
  '/admin/api/projects': {
    projects: [
      {
        id: PROJ,
        name: 'insight-mobile',
        platform: 'react-native',
        createdAt: iso(86_400_000 * 60),
      },
      {
        id: PROJ2,
        name: 'a-project-with-a-rather-long-name',
        platform: 'react-native',
        createdAt: iso(86_400_000 * 5),
      },
    ],
  },
  '/admin/api/users': {
    users: [
      {
        id: U1,
        email: 'takagi@golia.jp',
        role: 'superadmin',
        displayName: 'Takagi',
        createdAt: iso(86_400_000 * 90),
        lastLoginAt: iso(600_000),
        projects: [],
      },
      // Never logged in, no display name, scoped to one project.
      {
        id: U2,
        email: 'lihao@golia.jp',
        role: 'admin',
        displayName: null,
        createdAt: iso(86_400_000 * 3),
        lastLoginAt: null,
        projects: [PROJ],
      },
    ],
  },
  // Deliberately unconfigured: the "SMTP is off" banner and the
  // disabled test button only exist in this state.
  '/admin/api/smtp': { configured: false },
  '/admin/api/notification-prefs': {
    prefs: [
      {
        projectId: PROJ,
        projectName: 'insight-mobile',
        onNewIssue: true,
        onRegression: true,
      },
      {
        projectId: PROJ2,
        projectName: 'a-project-with-a-rather-long-name',
        onNewIssue: false,
        onRegression: true,
      },
    ],
  },
};

const health = {
  backend: {
    url: 'https://api.example.com/healthz',
    lastOk: true,
    lastStatus: 200,
    lastLatencyMs: 139,
    lastCheckedAt: iso(45_000),
    checks24h: 1436,
    ok24h: 1436,
  },
  lastEventAt: iso(t0),
  counts24h: { assert: 45_012, error: 1382, probe: 2, trace: 5120, warn: 402 },
  users24h: 980,
  platforms24h: { android: 77, ios: 269 },
  latestRelease: 'myapp@1.2.3+456',
  // The gap this page exists to show: iOS traffic, no dSYM.
  latestReleaseArtifacts: ['proguard', 'sourcemap'],
  replay24h: { eligible: 1382, withScreens: 41 },
};

const instruments = {
  asserts: [
    {
      name: 'cart.total matched the server',
      release: 'myapp@1.2.3+456',
      passCount: 44_988,
      failCount: 24,
      lastPassAt: iso(60_000),
      lastFailAt: iso(7_200_000),
    },
    // Never failed: the failure column has nothing to print.
    {
      name: 'session.token present',
      release: 'myapp@1.2.3+456',
      passCount: 12_004,
      failCount: 0,
      lastPassAt: iso(30_000),
      lastFailAt: null,
    },
  ],
  probes: [
    {
      ref: 'checkout-double-charge',
      issueId: ISSUE,
      lastSeenRelease: 'myapp@1.2.3+456',
      registeredAt: iso(86_400_000 * 6),
      lastFiredAt: iso(1_800_000),
      fireCount: 2,
    },
    // Silent since it was planted — the fix is holding, and this row
    // is the one that must not look like an error.
    {
      ref: 'nav-stack-leak',
      issueId: null,
      lastSeenRelease: null,
      registeredAt: iso(86_400_000 * 14),
      lastFiredAt: null,
      fireCount: 0,
    },
  ],
  traces: [
    {
      name: 'app.launch',
      eventCount: 5120,
      usersCount: 980,
      lastSeen: iso(120_000),
    },
    {
      name: 'checkout.submit',
      eventCount: 611,
      usersCount: 288,
      lastSeen: iso(900_000),
    },
  ],
  launch: [
    {
      release: 'myapp@1.2.3+456',
      samples: 5120,
      prewarmed: 214,
      p50: 1180,
      p90: 2440,
      p95: 3110,
    },
    // Too few real samples to have percentiles: nulls, not zeros.
    {
      release: 'myapp@1.2.2+441',
      samples: 3,
      prewarmed: 3,
      p50: null,
      p90: null,
      p95: null,
    },
  ],
};

const SUFFIX = [
  [/\/projects\/[^/]+\/instruments$/, () => instruments],
  [/\/projects\/[^/]+\/health$/, () => health],
  [/\/projects\/[^/]+\/environments$/, () => ({ environments: ['production', 'staging', 'development'] })],
  [/\/projects\/[^/]+\/context-keys$/, () => ({ keys: ['plan', 'tenant'] })],
  [/\/projects\/[^/]+\/context-values$/, () => ({ values: ['acme', 'globex', 'initech'] })],
  // Push: a project mid-integration — one provider configured, one
  // delivery failing for a nameable reason, devices quarantined.
  // The empty case (no credential, no send) is the state a new user
  // arrives in and the one the copy has to earn.
  // A project that is most of the way there: devices on a provider
  // with no credential, and nothing has ever called user().
  [
    /\/push\/readiness$/,
    () => ({
      live: 388,
      ready: false,
      checks: [
        { id: 'no-credential', level: 'blocked', data: { provider: 'fcm', devices: 300 } },
        { id: 'no-identity', level: 'warn', data: { live: 388 } },
        { id: 'no-traits', level: 'warn', data: { live: 388 } },
        { id: 'mass-quarantine', level: 'warn', data: { quarantined: 412, live: 388 } },
        { id: 'apns-mixed-env', level: 'info', data: { sandbox: 12, live: 388 } },
      ],
    }),
  ],
  [
    /\/push\/health$/,
    () => ({
      sent24h: 412,
      failed24h: 7,
      queued: 2,
      lastSendAt: iso(600_000),
      liveTokens: 388,
      identifiedTokens: 351,
      quarantinedTokens: 4,
      reasons: [
        { reason: 'BadDeviceToken', count: 5 },
        { reason: 'Unregistered', count: 2 },
      ],
    }),
  ],
  // Three devices covering the three states the table exists to tell
  // apart: metadata present and addressable, metadata present but
  // registered before `sentori.user()` ran, and a device that sent
  // none at all — which is what every row read until server 2.22.0.
  // The audience preview. The sweep opens this section with a
  // condition already in it, because an empty editor photographs as an
  // empty editor — the part worth looking at is a counted audience
  // next to the devices it picked.
  [
    /\/push\/audience\/preview$/,
    () => ({
      matched: 128,
      sample: [
        {
          id: 'dt1', provider: 'apns', addressable: true, userKeyTail: 'a91f3c',
          traits: { plan: 'pro', locale: 'ja-JP' },
          metadata: { appVersion: '4.10.0', channel: 'store' },
        },
        {
          id: 'dt2', provider: 'fcm', addressable: true, userKeyTail: '7d2e04',
          traits: { plan: 'team', locale: 'ja-JP' },
          metadata: { appVersion: '4.2.0', channel: 'beta' },
        },
      ],
    }),
  ],
  [
    /\/push\/audience\/send$/,
    () => ({ queued: 128 }),
  ],
  [
    /\/push\/devices$/,
    () => ({
      // `total` is larger than the rows returned on purpose: the
      // pager only exists because a project can have more devices
      // than a page, and a fixture that never exceeds one page
      // renders a control nobody can see.
      total: 388,
      offset: 0,
      devices: [
        {
          id: 'dt1', provider: 'apns', env: 'production',
          metadata: { appVersion: '4.2.1', channel: 'store', locale: 'ja-JP' },
          traits: { plan: 'pro', org: 'acme' },
          addressable: true,
          userKeyTail: 'a91f3c', badStreak: 0, revokedAt: null,
          lastSeenAt: iso(300_000), createdAt: iso(86_400_000 * 30),
          tokenTail: 'a91f3c',
        },
        {
          id: 'dt2', provider: 'fcm', env: null,
          metadata: { appVersion: '4.2.0', channel: 'beta' },
          traits: {},
          addressable: false, badStreak: 0, revokedAt: null,
          lastSeenAt: iso(3_600_000), createdAt: iso(86_400_000 * 3),
          tokenTail: '7d2e04',
        },
        {
          id: 'dt3', provider: 'apns', env: 'sandbox',
          metadata: {},
          addressable: true,
          userKeyTail: 'a91f3c', badStreak: 4, revokedAt: null,
          lastSeenAt: iso(86_400_000), createdAt: iso(86_400_000 * 9),
          tokenTail: 'ff10b8',
        },
      ],
    }),
  ],
  [
    /\/push\/credentials$/,
    () => ({
      credentials: [
        {
          id: 'pc1',
          kind: 'apns',
          // Deliberately the shape the old form produced: the
          // placeholder said `bundleId` and the worker reads `topic`,
          // so a credential filled in by following the example
          // exactly cannot be used. The server now says so on the
          // row; this is the fixture that renders it.
          config: { keyId: 'ABC123', teamId: 'DEF456', bundleId: 'com.example.app' },
          problem: { code: 'field-missing', field: 'topic' },
          created_at: iso(86_400_000 * 15),
          last_validated_at: iso(86_400_000),
          last_validate_status: 'ok',
        },
        // Never validated: the state a credential is in the moment it
        // is pasted, and the one the copy must not call an error.
        {
          id: 'pc2',
          kind: 'fcm',
          config: { projectId: 'demo-app' },
          created_at: iso(3_600_000),
          last_validated_at: null,
          last_validate_status: null,
        },
      ],
    }),
  ],
  [
    /\/push\/sends$/,
    () => ({
      total: 412,
      offset: 0,
      sends: [
        { id: 'ps1', token_id: 'dt1', provider: 'apns', status: 'failed',
          provider_outcome: '410', error: 'BadDeviceToken', retry_count: 3,
          payload: { title: '结账页已修复', body: '更新到 4.2.1 就好了' },
          payload: { title: '结账页已修复', body: '更新到 4.2.1 就好了' },
          created_at: iso(1_800_000), sent_at: null, next_attempt_at: iso(-600_000) },
        { id: 'ps2', token_id: 'dt2', provider: 'apns', status: 'sent',
          provider_outcome: '200', error: null, retry_count: 0,
          payload: { title: '新機能のお知らせ' },
          created_at: iso(3_600_000), sent_at: iso(3_599_000), next_attempt_at: null },
        { id: 'ps3', token_id: 'dt3', provider: 'fcm', status: 'queued',
          provider_outcome: null, error: null, retry_count: 0,
          payload: {},
          created_at: iso(120_000), sent_at: null, next_attempt_at: iso(0) },
      ],
    }),
  ],
  [
    /\/projects\/[^/]+\/tokens$/,
    () => ({
      tokens: [
        {
          id: 'tk1',
          name: 'production iOS',
          scope: 'ingest',
          last4: 'a91f',
          createdAt: iso(86_400_000 * 30),
          revokedAt: null,
        },
        {
          id: 'tk2',
          name: 'CI upload',
          scope: 'api',
          last4: null,
          createdAt: iso(86_400_000 * 55),
          revokedAt: iso(86_400_000 * 2),
        },
      ],
    }),
  ],
  [
    /\/releases\/[^/]+\/artifacts$/,
    () => ({
      artifacts: [
        {
          id: 'ar1',
          kind: 'sourcemap',
          name: 'index.android.bundle.map',
          content_hash: 'abc123',
          size_bytes: 4_194_304,
          created_at: iso(86_400_000),
        },
        // Stored, unreadable: the Hermes bytecode bundle uploaded
        // under `kind=sourcemap`, which is what insight had on two
        // releases while the light above it stayed green.
        {
          id: 'ar3',
          kind: 'sourcemap',
          name: 'index.android.bundle',
          content_hash: 'bad999',
          size_bytes: 9_154_716,
          created_at: iso(86_400_000),
          usable: false,
        },
        {
          id: 'ar2',
          kind: 'proguard',
          name: 'mapping.txt',
          content_hash: 'def456',
          size_bytes: 8_912_896,
          created_at: iso(86_400_000),
        },
      ],
    }),
  ],
  // rl1 hears from iOS only (its missing dsym is the light that must go
  // red, its missing proguard the one that must stay quiet); rl2 has no
  // traffic at all, the empty-array path.
  [
    /\/releases$/,
    () => ({
      releases: [
        {
          id: 'rl1',
          name: 'myapp@1.2.3+456',
          createdAt: iso(86_400_000),
          platforms: ['ios'],
        },
        {
          id: 'rl2',
          name: 'myapp@1.2.2+441',
          createdAt: iso(86_400_000 * 9),
          platforms: [],
        },
      ],
    }),
  ],
  [/\/issues\/[^/]+\/events$/, () => ({ events: occurrences })],
  [
    /\/issues\/[^/]+$/,
    (p) => {
      const id = p.split('/').pop();
      const found = issues.find((i) => i.id === id) ?? issues[0];
      return { ...found, activity, releases: issueReleases };
    },
  ],
  [/\/issues$/, () => ({ issues })],
  [/\/events\/[^/]+\/context$/, () => ({ events: contextEvents })],
  [/\/events\/[^/]+$/, () => eventDetail],
  [
    /\/audit$/,
    () => ({
      entries: [
        {
          id: 'au1',
          projectId: PROJ,
          actorUserId: U1,
          actorEmail: 'takagi@golia.jp',
          action: 'token.mint',
          targetType: 'token',
          targetId: 'tk1',
          payload: { scope: 'ingest' },
          createdAt: iso(7_200_000),
        },
        // An actor who has since been deleted: the row must still say
        // who it was, or say nothing, but never "null".
        {
          id: 'au2',
          projectId: null,
          actorUserId: null,
          actorEmail: null,
          action: 'user.delete',
          targetType: 'user',
          targetId: 'gone',
          payload: {},
          createdAt: iso(86_400_000 * 2),
        },
      ],
    }),
  ],
];

// Anything asked for and not answered. The sweep reads this at the
// end and fails on it: a page that renders its empty state because
// the mock returned `{}` looks exactly like a page with nothing to
// show, which is the most convincing way to miss a bug.
const unmocked = new Set();

// MOCK_FAIL=<substring> makes every matching path answer 500 with the
// shape the real server sends, so the error state of a page can be
// looked at rather than reasoned about. Every page has a loading and
// an empty state that a sweep renders by default; the error state is
// the one nothing ever renders, which is how the dashboard shipped
// with every API error displaying as ": ".
const FAIL = process.env.MOCK_FAIL || '';

createServer((req, res) => {
  const p = new URL(req.url, 'http://x').pathname;
  if (FAIL && p.includes(FAIL)) {
    res.statusCode = 500;
    res.setHeader('content-type', 'application/json');
    return res.end(
      JSON.stringify({
        error: 'upstream_unavailable',
        detail: 'the database refused the connection',
      }),
    );
  }
  if (p === '/__unmocked') {
    res.setHeader('content-type', 'application/json');
    return res.end(JSON.stringify({ unmocked: [...unmocked] }));
  }
  // The crash view fetches replay bytes by attachment ref. Both kinds
  // are NDJSON but they are NOT the same NDJSON: `screens` is one
  // encoded frame per line, `replay` is a wireframe keyframe plus
  // deltas. Serving one where the other was asked for renders as
  // "replay failed to load" — which is how this mock spent its first
  // sweep claiming a bug in the player.
  if (p.includes('/attachments/')) {
    res.setHeader('content-type', 'application/x-ndjson');
    res.end(p.endsWith('wireframe-demo') ? wireframeNdjson() : screensNdjson());
    return;
  }
  res.setHeader('content-type', 'application/json');
  if (p in EXACT) return res.end(JSON.stringify(EXACT[p]));
  for (const [re, make] of SUFFIX) {
    if (re.test(p)) return res.end(JSON.stringify(make(p)));
  }
  unmocked.add(p);
  process.stdout.write(`UNMOCKED ${p}\n`);
  res.end('{}');
}).listen(8080, () => console.log('mock api :8080'));
