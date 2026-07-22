import { afterEach, beforeEach, expect, test } from 'bun:test';

import {
  __resetRuntimeMetricsForTests,
  drainRuntimeMetricsForFlush,
} from '@goliapkg/sentori-core';

import {
  __forceNetworkEmitForTests,
  __peekNetworkCountersForTests,
  __resetNetworkBytesForTests,
  estimateRequestBytes,
  estimateResponseBytes,
  recordNetworkBytes,
} from '../runtime-metrics-network';

beforeEach(() => {
  __resetRuntimeMetricsForTests();
  __resetNetworkBytesForTests();
});

afterEach(() => __resetNetworkBytesForTests());

test('recordNetworkBytes: cumulates sent + received separately', () => {
  recordNetworkBytes(100, 200);
  recordNetworkBytes(50, 0);
  recordNetworkBytes(0, 25);
  const { sent, received } = __peekNetworkCountersForTests();
  expect(sent).toBe(150);
  expect(received).toBe(225);
});

test('force emit: flushes both counters into the ring + resets', () => {
  recordNetworkBytes(1234, 5678);
  __forceNetworkEmitForTests();

  const drained = drainRuntimeMetricsForFlush();
  expect(drained.length).toBe(2);
  const names = drained.map((m) => m.name).sort();
  expect(names).toEqual(['runtime.network.bytes_received', 'runtime.network.bytes_sent']);
  expect(drained.find((m) => m.name === 'runtime.network.bytes_sent')!.value).toBe(1234);
  expect(drained.find((m) => m.name === 'runtime.network.bytes_received')!.value).toBe(5678);

  const { sent, received } = __peekNetworkCountersForTests();
  expect(sent).toBe(0);
  expect(received).toBe(0);
});

test('force emit: skips zero counters (no empty metric emit)', () => {
  __forceNetworkEmitForTests();
  expect(drainRuntimeMetricsForFlush().length).toBe(0);
});

test('estimateRequestBytes: handles string body', () => {
  expect(estimateRequestBytes({ body: 'hello world' })).toBe(11);
  expect(estimateRequestBytes()).toBe(0);
  expect(estimateRequestBytes({})).toBe(0);
});

test('estimateRequestBytes: reads byteLength from ArrayBuffer-like body', () => {
  const buf = new Uint8Array([1, 2, 3, 4, 5]);
  expect(estimateRequestBytes({ body: buf })).toBe(5);
});

test('estimateResponseBytes: parses content-length header', () => {
  const h = new Headers({ 'content-length': '1024' });
  expect(estimateResponseBytes(h)).toBe(1024);
});

test('estimateResponseBytes: returns 0 when header missing (chunked / stripped)', () => {
  expect(estimateResponseBytes(new Headers())).toBe(0);
  expect(estimateResponseBytes(null)).toBe(0);
});

test('estimateResponseBytes: returns 0 on unparseable header (undercount-safe)', () => {
  const h = new Headers({ 'content-length': 'nope' });
  expect(estimateResponseBytes(h)).toBe(0);
});
