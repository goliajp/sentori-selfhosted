import { afterEach, describe, expect, test } from 'bun:test';

import { clearSpans, drainSpans } from '@goliapkg/sentori-core';

import { measureFn } from '../measure';

afterEach(() => {
  clearSpans();
});

describe('measureFn', () => {
  test('runs fn, returns result, emits an ok span', async () => {
    const r = await measureFn('addToCart', async () => 42);
    expect(r).toBe(42);
    const spans = drainSpans();
    expect(spans.length).toBe(1);
    expect(spans[0]!.op).toBe('sentori.measureFn');
    expect(spans[0]!.name).toBe('addToCart');
    expect(spans[0]!.status).toBe('ok');
  });

  test('supports sync fn too (Promise.resolve hides the difference)', async () => {
    const r = await measureFn('syncJob', () => 'hello');
    expect(r).toBe('hello');
    expect(drainSpans()[0]!.status).toBe('ok');
  });

  test('propagates thrown errors and marks span as error', async () => {
    await expect(
      measureFn('failing', async () => {
        throw new Error('nope');
      }),
    ).rejects.toThrow('nope');
    const spans = drainSpans();
    expect(spans.length).toBe(1);
    expect(spans[0]!.status).toBe('error');
    expect(spans[0]!.tags['error.message']).toBe('nope');
  });

  test('passes through caller tags', async () => {
    await measureFn('withTags', async () => 'ok', { tags: { region: 'jp' } });
    const spans = drainSpans();
    expect(spans[0]!.tags.region).toBe('jp');
  });
});
