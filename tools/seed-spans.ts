#!/usr/bin/env bun
//
// Phase 34 sub-D — synthetic span ingest.
//
// Posts N traces (each with M child spans) at /v1/spans:batch so the
// EXPLAIN baseline + dashboard trace-list work has a realistic shape
// without waiting on production traffic. Mirrors tools/seed-events.ts.
//
// Usage:
//   bun tools/seed-spans.ts \
//     --token st_pk_dev0000000000000000000000 \
//     --traces 500 --spans-per-trace 200 \
//     --ingest-url http://localhost:8080
//
// Defaults: 500 traces × 200 spans = 100k spans total.

interface Args {
  token: string
  traces: number
  spansPerTrace: number
  ingestUrl: string
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
  const token = get('token')
  if (!token) {
    console.error('--token is required')
    process.exit(2)
  }
  return {
    token,
    traces: Number.parseInt(get('traces') ?? '500', 10),
    spansPerTrace: Number.parseInt(
      get('spans-per-trace') ?? get('spansPerTrace') ?? '200',
      10,
    ),
    ingestUrl:
      get('ingest-url') ?? get('ingestUrl') ?? 'https://ingest.sentori.golia.jp',
  }
}

function uuidV7(): string {
  const tsMs = BigInt(Math.floor(Date.now()))
  const tsHex = tsMs.toString(16).padStart(12, '0')
  const rand = crypto.getRandomValues(new Uint8Array(10))
  let hex = ''
  for (const b of rand) hex += b.toString(16).padStart(2, '0')
  return `${tsHex.slice(0, 8)}-${tsHex.slice(8, 12)}-7${hex.slice(0, 3)}-${(0x80 | (rand[2]! & 0x3f)).toString(16)}${hex.slice(4, 6)}-${hex.slice(6, 18)}`
}

// Realistic op distribution. Roughly mirrors a Node API with a DB and
// a cache, plus client-side render spans for variety.
const OP_POOL = [
  'http.client',
  'http.server',
  'db.query',
  'db.transaction',
  'cache.get',
  'cache.set',
  'react.render',
  'react.navigation',
] as const

const STATUS_POOL = ['ok', 'ok', 'ok', 'ok', 'ok', 'ok', 'ok', 'ok', 'error', 'cancelled'] as const

function pick<T>(arr: readonly T[]): T {
  return arr[Math.floor(Math.random() * arr.length)]!
}

function buildTrace(traceId: string, spansPerTrace: number): object[] {
  const rootSpanId = uuidV7()
  const baseStart = new Date(
    Date.now() - Math.floor(Math.random() * 7 * 24 * 3600 * 1000),
  )
  // Bias 60% of trace activity to the last 24h so dashboard default
  // windows have content.
  const biased =
    Math.random() < 0.6
      ? new Date(Date.now() - Math.floor(Math.random() * 24 * 3600 * 1000))
      : baseStart

  const rootOp = Math.random() < 0.7 ? 'http.server' : pick(OP_POOL)
  const rootName = `${rootOp} /api/${pick(['users', 'orders', 'pay', 'sessions', 'health'])}`
  const rootDuration = 50 + Math.floor(Math.random() * 950)
  const root = {
    id: rootSpanId,
    traceId,
    parentSpanId: null,
    op: rootOp,
    name: rootName,
    startedAt: biased.toISOString(),
    durationMs: rootDuration,
    status: pick(STATUS_POOL),
    tags: { 'http.method': pick(['GET', 'POST', 'PATCH', 'DELETE']) },
  }

  const spans: object[] = [root]
  for (let i = 1; i < spansPerTrace; i++) {
    const childStart = new Date(
      biased.getTime() + Math.floor(Math.random() * rootDuration),
    )
    spans.push({
      id: uuidV7(),
      traceId,
      parentSpanId: rootSpanId,
      op: pick(OP_POOL.filter((o) => o !== 'http.server')),
      name: `child-${i}`,
      startedAt: childStart.toISOString(),
      durationMs: 1 + Math.floor(Math.random() * 100),
      status: pick(STATUS_POOL),
      tags: {},
    })
  }
  return spans
}

async function postBatch(
  spans: object[],
  args: Args,
): Promise<{ accepted: number; rejected: number }> {
  const resp = await fetch(
    `${args.ingestUrl.replace(/\/$/, '')}/v1/spans:batch`,
    {
      body: JSON.stringify({ spans }),
      headers: {
        Authorization: `Bearer ${args.token}`,
        'Content-Type': 'application/json',
        'Sentori-Sdk': 'seed-spans/0.0.1',
      },
      method: 'POST',
    },
  )
  if (!resp.ok) {
    const body = await resp.text().catch(() => '<no body>')
    throw new Error(`batch failed: ${resp.status} ${body.slice(0, 200)}`)
  }
  const result = (await resp.json()) as { accepted: number; rejected: number }
  return result
}

async function main() {
  const args = parseArgs()
  console.log(
    `[seed-spans] target=${args.ingestUrl} traces=${args.traces} spans-per-trace=${args.spansPerTrace} (total ≈ ${args.traces * args.spansPerTrace})`,
  )

  const BATCH_SIZE = 200
  let buffer: object[] = []
  let totalAccepted = 0
  let totalRejected = 0
  const startedAt = Date.now()

  for (let t = 0; t < args.traces; t++) {
    const traceSpans = buildTrace(uuidV7(), args.spansPerTrace)
    for (const span of traceSpans) {
      buffer.push(span)
      if (buffer.length >= BATCH_SIZE) {
        const { accepted, rejected } = await postBatch(buffer, args)
        totalAccepted += accepted
        totalRejected += rejected
        buffer = []
      }
    }
    if ((t + 1) % 50 === 0) {
      const rate = totalAccepted / ((Date.now() - startedAt) / 1000)
      console.log(
        `  ${t + 1}/${args.traces} traces (${totalAccepted} spans, ${rate.toFixed(0)} sp/s)`,
      )
    }
  }
  if (buffer.length > 0) {
    const { accepted, rejected } = await postBatch(buffer, args)
    totalAccepted += accepted
    totalRejected += rejected
  }

  const elapsed = (Date.now() - startedAt) / 1000
  console.log(
    `\ndone — accepted ${totalAccepted}, rejected ${totalRejected} in ${elapsed.toFixed(1)}s (${(totalAccepted / elapsed).toFixed(0)} sp/s)`,
  )
}

void main()
