#!/usr/bin/env bun
/*
 * Phase 33 sub-C: bun-native load test for the ingest endpoints.
 *
 * Replaces what would have been a `k6 run` invocation — the install
 * footprint for k6 (Go binary + Grafana stack to view the output)
 * was disproportionate to "post events at a steady rate for a while
 * and check the latency distribution." This script does the same
 * with the same scheduling model.
 *
 * Usage:
 *   bun tools/load-test.ts \
 *     --token "$SENTORI_TOKEN" \
 *     --ingest-url http://localhost:8080 \
 *     --rate 50 --duration 60
 *
 * Targets all four hot-path ingest endpoints in rotation:
 *
 *   POST /v1/events       — single event
 *   POST /v1/events:batch — batched events
 *   POST /v1/sessions     — session ping
 *   POST /v1/deploys      — deploy notification
 *
 * Output: per-endpoint P50 / P95 / P99 latency + error rate, plus
 * an overall summary.
 *
 * Methodology: open-loop scheduler. At t=0 fire the first batch of
 * requests; every 1/rate seconds, fire another. Late requests do NOT
 * stack up — the schedule is fixed regardless of how slow the server
 * is. This is what you want for SLO measurement: you're measuring
 * the server, not the wave you generated.
 */

type Args = {
  ingestUrl: string
  token: string
  rate: number
  duration: number
}

function parseArgs(argv: string[]): Args {
  const args: Partial<Args> = {}
  for (let i = 2; i < argv.length; i += 2) {
    const key = argv[i]
    const val = argv[i + 1]
    if (!key?.startsWith('--') || val === undefined) continue
    const k = key.slice(2).replace(/-([a-z])/g, (_, c) => c.toUpperCase()) as keyof Args
    if (k === 'rate' || k === 'duration') {
      args[k] = Number(val)
    } else {
      ;(args as Record<string, string>)[k] = val
    }
  }
  if (!args.token || !args.ingestUrl) {
    console.error('Usage: bun tools/load-test.ts --token TOKEN --ingest-url URL [--rate N] [--duration S]')
    process.exit(1)
  }
  return {
    ingestUrl: args.ingestUrl.replace(/\/$/, ''),
    token: args.token,
    rate: args.rate ?? 50,
    duration: args.duration ?? 60,
  }
}

function uuidv7(): string {
  const ms = BigInt(Date.now())
  const a = ms.toString(16).padStart(12, '0')
  const r = crypto.getRandomValues(new Uint8Array(10))
  const hex = (n: number) => n.toString(16).padStart(2, '0')
  let h = ''
  for (const b of r) h += hex(b)
  return `${a.slice(0, 8)}-${a.slice(8, 12)}-7${h.slice(0, 3)}-${(0x80 | (r[2]! & 0x3f)).toString(16)}${h.slice(4, 6)}-${h.slice(6, 18)}`
}

function buildPayload(kind: 'event' | 'batch' | 'session' | 'deploy'): object {
  const base = {
    id: uuidv7(),
    timestamp: new Date().toISOString(),
    release: 'loadtest@0.0.1',
    environment: 'loadtest',
  }
  switch (kind) {
    case 'event':
      return {
        ...base,
        kind: 'error',
        platform: 'javascript',
        device: { os: 'other', osVersion: '0' },
        app: { version: '0.0.1' },
        error: {
          type: 'LoadTestErr',
          message: 'synthetic load-test event',
          stack: [
            { file: 'load-test.ts', function: 'fire', line: 42, inApp: true },
            { file: 'load-test.ts', function: 'main', line: 7, inApp: true },
          ],
        },
      }
    case 'batch':
      return {
        events: Array.from({ length: 5 }, () => buildPayload('event')),
      }
    case 'session':
      return {
        id: uuidv7(),
        startedAt: new Date().toISOString(),
        release: 'loadtest@0.0.1',
        environment: 'loadtest',
        status: 'ok',
        durationMs: 10000,
      }
    case 'deploy':
      return {
        release: 'loadtest@0.0.1',
        environment: 'loadtest',
      }
  }
}

type Sample = { kind: string; latencyMs: number; status: number; ok: boolean }

const ENDPOINTS = [
  { kind: 'event' as const, path: '/v1/events' },
  { kind: 'batch' as const, path: '/v1/events:batch' },
  { kind: 'session' as const, path: '/v1/sessions' },
  { kind: 'deploy' as const, path: '/v1/deploys' },
]

async function fireOne(args: Args, idx: number): Promise<Sample> {
  const ep = ENDPOINTS[idx % ENDPOINTS.length]!
  const body = JSON.stringify(buildPayload(ep.kind))
  const start = performance.now()
  try {
    const resp = await fetch(`${args.ingestUrl}${ep.path}`, {
      body,
      headers: {
        Authorization: `Bearer ${args.token}`,
        'Content-Type': 'application/json',
        'Sentori-Sdk': 'load-test/0.0.1',
      },
      method: 'POST',
    })
    return {
      kind: ep.kind,
      latencyMs: performance.now() - start,
      ok: resp.ok,
      status: resp.status,
    }
  } catch {
    return { kind: ep.kind, latencyMs: performance.now() - start, ok: false, status: 0 }
  }
}

function percentile(values: number[], p: number): number {
  if (values.length === 0) return 0
  const sorted = [...values].sort((a, b) => a - b)
  const idx = Math.min(Math.floor(sorted.length * p), sorted.length - 1)
  return sorted[idx]!
}

async function main() {
  const args = parseArgs(process.argv)
  console.log(
    `[load-test] target=${args.ingestUrl}  rate=${args.rate}/s  duration=${args.duration}s  total=${args.rate * args.duration}`,
  )
  const intervalMs = 1000 / args.rate
  const startedAt = performance.now()
  const samples: Sample[] = []
  const inFlight: Promise<Sample>[] = []

  let i = 0
  const total = args.rate * args.duration
  while (i < total) {
    const targetT = i * intervalMs
    const drift = performance.now() - startedAt - targetT
    if (drift < 0) await Bun.sleep(-drift)
    const idx = i
    const promise = fireOne(args, idx).then((s) => {
      samples.push(s)
      return s
    })
    inFlight.push(promise)
    i++
    if (i % args.rate === 0) {
      process.stdout.write(
        `\r[load-test] ${i}/${total} fired (${(i / (args.duration * args.rate) * 100).toFixed(0)}%)`,
      )
    }
  }
  // drain
  await Promise.all(inFlight)
  console.log('\n[load-test] all requests drained')

  // Aggregate
  console.log()
  console.log(`${'endpoint'.padEnd(10)} ${'count'.padStart(6)} ${'errors'.padStart(7)} ${'P50'.padStart(7)} ${'P95'.padStart(7)} ${'P99'.padStart(7)} ${'max'.padStart(7)}`)
  console.log('-'.repeat(58))
  const byKind = new Map<string, Sample[]>()
  for (const s of samples) {
    const arr = byKind.get(s.kind) ?? []
    arr.push(s)
    byKind.set(s.kind, arr)
  }
  for (const ep of ENDPOINTS) {
    const ss = byKind.get(ep.kind) ?? []
    const lat = ss.map((s) => s.latencyMs)
    const errs = ss.filter((s) => !s.ok).length
    console.log(
      `${ep.kind.padEnd(10)} ${String(ss.length).padStart(6)} ${String(errs).padStart(7)} ${percentile(lat, 0.5).toFixed(1).padStart(7)} ${percentile(lat, 0.95).toFixed(1).padStart(7)} ${percentile(lat, 0.99).toFixed(1).padStart(7)} ${Math.max(...lat, 0).toFixed(1).padStart(7)}`,
    )
  }
  const allLat = samples.map((s) => s.latencyMs)
  const allErrs = samples.filter((s) => !s.ok).length
  console.log('-'.repeat(58))
  console.log(
    `${'TOTAL'.padEnd(10)} ${String(samples.length).padStart(6)} ${String(allErrs).padStart(7)} ${percentile(allLat, 0.5).toFixed(1).padStart(7)} ${percentile(allLat, 0.95).toFixed(1).padStart(7)} ${percentile(allLat, 0.99).toFixed(1).padStart(7)} ${Math.max(...allLat, 0).toFixed(1).padStart(7)}`,
  )
  console.log()
  console.log('latencies in ms')
}

void main()
