/**
 * v2.3 — beforeSend hook unit coverage for the RN SDK.
 *
 * We exercise the dispatcher directly via the
 * `__applyBeforeSendForTests` test hook rather than running the full
 * captureException pipeline. The behaviour-level coverage of
 * captureException's enqueue path lives in the JS SDK's `sdk.test.ts`
 * (which mocks fetch); the dispatcher itself is the only RN-specific
 * piece (RN uses the same NEVER-rule policy as JS, but the
 * one-shot warn flag is per-SDK so we verify it in both).
 */
import { describe, expect, test } from 'bun:test';

import { __applyBeforeSendForTests as applyBeforeSend } from '../capture';
import type { Event } from '../types';

function freshEvent(message: string): Event {
  return {
    id: '019eaa00-0000-7000-8000-000000000001',
    timestamp: '2026-06-03T07:00:00.000Z',
    kind: 'message',
    platform: 'javascript',
    release: 'app@1.0.0+1',
    environment: 'test',
    device: { os: 'ios' },
    app: { version: '1.0.0', build: '1' },
    user: null,
    tags: {},
    breadcrumbs: [],
    message,
  } as Event;
}

describe('applyBeforeSend (RN, v2.3)', () => {
  test('no hook → returns event unchanged', () => {
    const ev = freshEvent('passthrough');
    expect(applyBeforeSend(ev, undefined)).toBe(ev);
  });

  test('hook mutates: returned event is what the caller sees', () => {
    const ev = freshEvent('please scrub');
    const out = applyBeforeSend(ev, (e) => ({
      ...e,
      tags: { ...e.tags, scrubbed: '1' },
    }));
    expect(out).not.toBeNull();
    expect(out!.tags?.scrubbed).toBe('1');
  });

  test('hook returns null → dispatcher returns null (caller drops)', () => {
    const ev = freshEvent('drop me');
    expect(applyBeforeSend(ev, () => null)).toBeNull();
  });

  test('hook throws → dispatcher returns the original event (NEVER rule)', () => {
    const ev = freshEvent('survives bad hook');
    const out = applyBeforeSend(ev, () => {
      throw new Error('hook boom');
    });
    expect(out).toBe(ev);
  });

  test('hook returns non-event garbage → dispatcher returns the original event', () => {
    const ev = freshEvent('survives bad return');
    const out = applyBeforeSend(
      ev,
      // @ts-expect-error — host returns garbage; v2.3 contract is "fall back unmodified"
      () => 42,
    );
    expect(out).toBe(ev);
  });
});
