import { beforeEach, expect, test } from 'bun:test'

import {
  RuntimeMetricBuffer,
  __peekRuntimeMetricsSize,
  __resetRuntimeMetricsForTests,
  drainRuntimeMetricsForFlush,
  emitMetric,
  rebufferRuntimeMetrics,
} from '../runtime-metrics.js'

beforeEach(() => __resetRuntimeMetricsForTests())

test('emit: round-trips name + value + tags into the ring', () => {
  emitMetric('runtime.fps.p50', 58, { release: 'app@1.0.0' })
  emitMetric('runtime.heap.used_bytes', 12345678)
  expect(__peekRuntimeMetricsSize()).toBe(2)

  const drained = drainRuntimeMetricsForFlush()
  expect(drained.length).toBe(2)
  expect(drained[0]!.name).toBe('runtime.fps.p50')
  expect(drained[0]!.value).toBe(58)
  expect(drained[0]!.tags).toEqual({ release: 'app@1.0.0' })
  expect(drained[1]!.name).toBe('runtime.heap.used_bytes')
  expect(drained[1]!.tags).toBeUndefined()
  // Ring is empty after drain.
  expect(__peekRuntimeMetricsSize()).toBe(0)
})

test('emit: drops malformed input silently (NEVER rule)', () => {
  // Empty name → silent drop
  emitMetric('', 1)
  // Non-finite value → silent drop
  emitMetric('runtime.x', Infinity)
  emitMetric('runtime.x', NaN)
  // > 16 tag keys → silent drop
  const tooMany: Record<string, string> = {}
  for (let i = 0; i < 20; i++) tooMany[`k${i}`] = 'v'
  emitMetric('runtime.x', 1, tooMany)
  // Tag value too long → silent drop
  emitMetric('runtime.x', 1, { k: 'v'.repeat(500) })
  // Tag key too long → silent drop
  emitMetric('runtime.x', 1, { ['k'.repeat(50)]: 'v' })

  expect(__peekRuntimeMetricsSize()).toBe(0)
})

test('emit: accepts a valid payload after rejecting malformed', () => {
  emitMetric('', 1) // rejected
  emitMetric('runtime.fps.p50', 60) // accepted
  expect(__peekRuntimeMetricsSize()).toBe(1)
})

test('ring cap: drops overflow + reports count on drain', () => {
  // Use a tiny custom ring to exercise overflow without 10k emits.
  const ring = new RuntimeMetricBuffer(3)
  ring.push({ name: 'a', value: 1, ts: '2026-06-03T00:00:00Z' })
  ring.push({ name: 'b', value: 2, ts: '2026-06-03T00:00:00Z' })
  ring.push({ name: 'c', value: 3, ts: '2026-06-03T00:00:00Z' })
  ring.push({ name: 'd', value: 4, ts: '2026-06-03T00:00:00Z' }) // dropped
  ring.push({ name: 'e', value: 5, ts: '2026-06-03T00:00:00Z' }) // dropped

  expect(ring.size()).toBe(3)
  expect(ring.takeDroppedCount()).toBe(2)
  // takeDroppedCount resets the counter
  expect(ring.takeDroppedCount()).toBe(0)
})

test('rebuffer: failed-flush points return to the ring', () => {
  emitMetric('runtime.fps.p50', 60)
  emitMetric('runtime.fps.p50', 58)
  const batch = drainRuntimeMetricsForFlush()
  expect(batch.length).toBe(2)
  expect(__peekRuntimeMetricsSize()).toBe(0)

  // Simulate a network failure: rebuffer.
  rebufferRuntimeMetrics(batch)
  expect(__peekRuntimeMetricsSize()).toBe(2)
})

test('drain: idempotent — repeated drain returns empty', () => {
  emitMetric('runtime.x', 1)
  expect(drainRuntimeMetricsForFlush().length).toBe(1)
  expect(drainRuntimeMetricsForFlush().length).toBe(0)
  expect(drainRuntimeMetricsForFlush().length).toBe(0)
})
