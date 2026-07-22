#!/usr/bin/env bun
//
// Phase 30 sub-C — synthetic ingest load.
//
// Posts N random events at /v1/events:batch so dashboard performance
// work (sub-D EXPLAIN, sub-E index pass) has a realistic shape to
// measure against without waiting on production users.
//
// Usage:
//   bun tools/seed-events.ts \
//     --token st_pk_dev0000000000000000000000 \
//     --events 5000 --users 200 --releases 10 \
//     --include-anr --include-regression \
//     --admin-token st_pk_admin... \
//     --project-id 019508a0-... \
//     --ingest-url http://localhost:8080
//
// Defaults: 5000 events, 200 users, 10 releases, no ANR, no regression,
// ingest URL https://ingest.sentori.golia.jp.

interface Args {
  token: string
  events: number
  users: number
  releases: number
  issues: number
  ingestUrl: string
  apiUrl: string | null
  adminToken: string | null
  projectId: string | null
  includeAnr: boolean
  includeRegression: boolean
}

function parseArgs(): Args {
  const argv = process.argv.slice(2)
  const get = (name: string, defaultVal?: string): string | null => {
    const flag = `--${name}`
    for (let i = 0; i < argv.length; i++) {
      if (argv[i] === flag) return argv[i + 1] ?? null
      const prefix = `--${name}=`
      if (argv[i]?.startsWith(prefix)) return argv[i]!.slice(prefix.length)
    }
    return defaultVal ?? null
  }
  const flag = (name: string): boolean => argv.includes(`--${name}`)

  const token = get('token')
  if (!token) {
    console.error('--token is required (a Sentori project public token)')
    process.exit(2)
  }
  const ingestUrl =
    get('ingest-url') ?? get('ingestUrl') ?? 'https://ingest.sentori.golia.jp'
  return {
    token,
    events: Number.parseInt(get('events') ?? '5000', 10),
    users: Number.parseInt(get('users') ?? '200', 10),
    releases: Number.parseInt(get('releases') ?? '10', 10),
    issues: Number.parseInt(get('issues') ?? '100', 10),
    ingestUrl,
    apiUrl:
      get('api-url') ??
      get('apiUrl') ??
      ingestUrl.replace('ingest.', 'api.'),
    adminToken: get('admin-token') ?? get('adminToken'),
    projectId: get('project-id') ?? get('projectId'),
    includeAnr: flag('include-anr'),
    includeRegression: flag('include-regression'),
  }
}

const ERROR_TYPES = [
  { type: 'TypeError', platform: 'javascript', fn: 'render', file: 'src/App.tsx' },
  { type: 'ReferenceError', platform: 'javascript', fn: 'init', file: 'src/main.tsx' },
  { type: 'NetworkError', platform: 'javascript', fn: 'fetch', file: 'src/api/client.ts' },
  { type: 'RangeError', platform: 'javascript', fn: 'parsePage', file: 'src/lib/pagination.ts' },
  { type: 'NullPointerException', platform: 'android', fn: 'onCreate', file: 'MainActivity.kt' },
  { type: 'IllegalStateException', platform: 'android', fn: 'render', file: 'Renderer.kt' },
  { type: 'NoSuchElementException', platform: 'android', fn: 'next', file: 'Iterator.kt' },
  { type: 'NSGenericException', platform: 'ios', fn: '-[VC viewDidLoad]', file: 'ViewController.swift' },
  { type: 'NSInvalidArgumentException', platform: 'ios', fn: 'configure', file: 'Config.swift' },
  { type: 'NSRangeException', platform: 'ios', fn: 'objectAtIndex', file: 'Array.swift' },
] as const

const ENV_WEIGHTS: Array<[string, number]> = [
  ['prod', 0.7],
  ['staging', 0.2],
  ['dev', 0.1],
]

function pick<T>(xs: readonly T[]): T {
  return xs[Math.floor(Math.random() * xs.length)]!
}
function weightedPick<T>(xs: Array<[T, number]>): T {
  const r = Math.random()
  let cum = 0
  for (const [v, w] of xs) {
    cum += w
    if (r < cum) return v
  }
  return xs[xs.length - 1]![0]
}

function uuidV7(timestampMs: number): string {
  const ts = BigInt(Math.floor(timestampMs))
  const timeHex = ts.toString(16).padStart(12, '0').slice(-12)
  const rand = new Uint8Array(10)
  crypto.getRandomValues(rand)
  rand[0] = (rand[0]! & 0x0f) | 0x70 // version 7
  rand[2] = (rand[2]! & 0x3f) | 0x80 // RFC 4122 variant
  const hex = Array.from(rand)
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('')
  return [
    timeHex.slice(0, 8),
    timeHex.slice(8, 12),
    hex.slice(0, 4),
    hex.slice(4, 8),
    hex.slice(8, 20),
  ].join('-')
}

function deviceFor(platform: string): Record<string, unknown> {
  if (platform === 'ios') {
    return {
      os: 'ios',
      osVersion: pick(['17.4', '17.5', '18.0', '18.1']),
      model: pick(['iPhone15,2', 'iPhone16,1', 'iPhone17,3']),
      locale: pick(['en-US', 'ja-JP', 'de-DE']),
    }
  }
  if (platform === 'android') {
    return {
      os: 'android',
      osVersion: pick(['13', '14', '15']),
      model: pick(['Pixel 8', 'Pixel 9 Pro', 'Galaxy S24']),
      locale: pick(['en-US', 'ja-JP', 'de-DE']),
    }
  }
  return {
    os: 'web',
    osVersion: pick(['macOS 14.5', 'Windows 11', 'Ubuntu 24.04']),
    model: pick(['MacBook Pro', 'ThinkPad X1', 'XPS 15']),
    locale: pick(['en-US', 'ja-JP', 'de-DE']),
  }
}

interface GenContext {
  releases: string[]
  userIds: string[]
}

function makeEvent(
  ctx: GenContext,
  includeAnr: boolean,
  issueDiversity: number,
): Record<string, unknown> {
  // Fingerprint = sha256(error.type + frame.fn + frame.file). For
  // issue-shape diversity we deterministically synthesize a (fn, file)
  // pair per "issue id" 0..N-1, then pick error.type from the pool
  // indexed by the same id. That gives ~N distinct fingerprints
  // regardless of the ERROR_TYPES pool size, which is what sub-D's
  // 5k-event / ~1k-issue baseline needs.
  const issueId = Math.floor(Math.random() * issueDiversity)
  const proto = ERROR_TYPES[issueId % ERROR_TYPES.length]!
  const isAnr = includeAnr && Math.random() < 0.05
  const platform =
    isAnr && Math.random() < 0.5 ? 'ios' : isAnr ? 'android' : proto.platform
  const environment = weightedPick(ENV_WEIGHTS)
  const release = pick(ctx.releases)
  const userId = Math.random() < 0.85 ? pick(ctx.userIds) : null

  // Spread the timestamp across the last 7 days, with a heavier tail
  // toward "today" so the dashboard's default 24h window has more to
  // grind on.
  const nowMs = Date.now()
  const ageMs =
    Math.random() < 0.6
      ? Math.random() * 86_400_000 // last 24h
      : Math.random() * 7 * 86_400_000 // last 7 days
  const tsMs = nowMs - ageMs

  // Per-issue synthesized stack frame so issueId fully determines the
  // fingerprint.
  const fnSlot = issueId
  const fileSlot = Math.floor(issueId / 10)
  return {
    id: uuidV7(tsMs),
    timestamp: new Date(tsMs).toISOString(),
    kind: isAnr ? 'anr' : 'error',
    platform,
    release,
    environment,
    device: deviceFor(platform),
    app: { version: release.split('@')[1]?.split('+')[0] ?? '1.0.0', build: '1' },
    user: userId ? { id: userId } : null,
    tags: {
      synthetic: 'seed-events',
      ...(isAnr ? { source: 'sentori.hangWatchdog' } : {}),
    },
    breadcrumbs: [],
    error: {
      type: proto.type,
      message: isAnr
        ? `Main thread blocked for ≥ ${Math.floor(2000 + Math.random() * 3000)} ms`
        : `${proto.type}: ${pick(['cannot read property', 'undefined is not a function', 'index out of range', 'network unreachable'])}`,
      stack: [
        {
          function: `${proto.fn}_v${fnSlot}`,
          file: `${proto.file.replace(/(\.[a-z]+)$/, `_v${fileSlot}$1`)}`,
          line: Math.floor(10 + Math.random() * 200),
          inApp: true,
        },
        {
          function: 'callPath',
          file: 'lib/runtime.js',
          line: Math.floor(10 + Math.random() * 1000),
          inApp: false,
        },
      ],
      cause: null,
    },
    fingerprint: [],
  }
}

async function postBatch(events: object[], args: Args): Promise<void> {
  const resp = await fetch(`${args.ingestUrl.replace(/\/$/, '')}/v1/events:batch`, {
    method: 'POST',
    headers: {
      'authorization': `Bearer ${args.token}`,
      'content-type': 'application/json',
      'sentori-sdk': 'seed-events/0.1',
    },
    body: JSON.stringify({ events }),
  })
  if (!resp.ok) {
    const body = await resp.text()
    throw new Error(`batch failed: ${resp.status} ${body.slice(0, 200)}`)
  }
}

async function simulateRegression(args: Args, fingerprintErrorType: string): Promise<boolean> {
  if (!args.adminToken || !args.projectId || !args.apiUrl) {
    return false
  }
  // Find an existing issue with this error_type and PATCH it to
  // status=resolved, then re-post a single matching event so the
  // ingest path's regression detector flips it back to "regressed".
  try {
    const listResp = await fetch(
      `${args.apiUrl.replace(/\/$/, '')}/admin/api/projects/${args.projectId}/issues?status=active`,
      { headers: { authorization: `Bearer ${args.adminToken}` } },
    )
    if (!listResp.ok) return false
    const list = (await listResp.json()) as Array<{
      id: string
      errorType: string
    }>
    const target = list.find((i) => i.errorType === fingerprintErrorType)
    if (!target) return false
    const patchResp = await fetch(
      `${args.apiUrl.replace(/\/$/, '')}/admin/api/projects/${args.projectId}/issues/${target.id}`,
      {
        method: 'PATCH',
        headers: {
          authorization: `Bearer ${args.adminToken}`,
          'content-type': 'application/json',
        },
        body: JSON.stringify({ status: 'resolved' }),
      },
    )
    return patchResp.ok
  } catch {
    return false
  }
}

async function main() {
  const args = parseArgs()
  console.log('seed-events config:', {
    events: args.events,
    users: args.users,
    releases: args.releases,
    ingestUrl: args.ingestUrl,
    includeAnr: args.includeAnr,
    includeRegression: args.includeRegression,
  })

  const ctx: GenContext = {
    releases: Array.from({ length: args.releases }, (_, i) =>
      `myapp@1.${i}.0+${(i + 1) * 100}`,
    ),
    userIds: Array.from({ length: args.users }, (_, i) => `u_${i.toString().padStart(4, '0')}`),
  }

  const BATCH_SIZE = 500
  let posted = 0
  let regressionsFired = 0
  const startedAt = Date.now()
  let nextBatch: object[] = []

  for (let i = 0; i < args.events; i++) {
    nextBatch.push(makeEvent(ctx, args.includeAnr, args.issues))
    if (nextBatch.length === BATCH_SIZE) {
      await postBatch(nextBatch, args)
      posted += nextBatch.length
      nextBatch = []
      if (posted % 1000 === 0) {
        const rate = posted / ((Date.now() - startedAt) / 1000)
        console.log(`  ${posted} / ${args.events} (${rate.toFixed(0)} ev/s)`)
      }
    }
  }
  if (nextBatch.length > 0) {
    await postBatch(nextBatch, args)
    posted += nextBatch.length
  }

  // Regression simulation: pick ~3% of error types to resolve, then
  // re-post one event each. Server flips them back to "regressed".
  if (args.includeRegression) {
    if (!args.adminToken || !args.projectId) {
      console.warn(
        '--include-regression skipped: needs --admin-token + --project-id',
      )
    } else {
      const sample = ERROR_TYPES.slice(0, Math.max(1, Math.floor(ERROR_TYPES.length * 0.3)))
      for (const proto of sample) {
        const resolved = await simulateRegression(args, proto.type)
        if (resolved) {
          await postBatch([makeEvent(ctx, false, args.issues)], args)
          regressionsFired++
        }
      }
    }
  }

  const elapsedSec = (Date.now() - startedAt) / 1000
  console.log()
  console.log(
    `done — posted ${posted} events in ${elapsedSec.toFixed(1)}s (${(posted / elapsedSec).toFixed(0)} ev/s)`,
  )
  if (args.includeRegression) {
    console.log(`regressions fired: ${regressionsFired}`)
  }
}

main().catch((e) => {
  console.error('seed-events failed:', e)
  process.exit(1)
})
