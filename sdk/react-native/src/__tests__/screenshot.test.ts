import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';

import { setConfig, __resetForTests as resetConfig } from '../config';
import { uploadAttachment } from '../transport';

// Unit coverage for the upload pipeline.
//
// `captureScreenshot()` itself goes through the native module
// (v0.7.3+) which doesn't exist in the bun:test runtime, so we
// test it indirectly: `uploadAttachment` is the non-RN-API surface
// and that's what we hit hardest. The native render + mask redaction
// is exercised by the iOS XCTest / Android instrumentation tests.

const origFetch = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = origFetch;
  resetConfig();
});
beforeEach(() => {
  globalThis.fetch = origFetch;
  setConfig({
    enabled: true,
    environment: 'test',
    ingestUrl: 'http://localhost:18080',
    release: 'app@1.0.0+1',
    screenshotsEnabled: true,
    token: 'st_pk_test',
  });
});

describe('uploadAttachment', () => {
  test('hits POST /v1/events/<id>/attachments/<kind> with the bearer token', async () => {
    const seen: { method?: string; url?: string; auth?: null | string } = {};
    globalThis.fetch = mock(async (url: Request | string | URL, init?: RequestInit) => {
      seen.url = String(url);
      seen.method = init?.method;
      // Reach the Bearer header off the Headers/Init shape used here.
      const headers = (init?.headers ?? {}) as Record<string, string>;
      seen.auth = headers.Authorization;
      return new Response(
        JSON.stringify({
          kind: 'screenshot',
          mediaType: 'image/jpeg',
          refId: '019e3000-7000-7000-8000-000000000001',
          sizeBytes: 4,
        }),
        { headers: { 'content-type': 'application/json' }, status: 201 },
      );
    }) as typeof fetch;

    const out = await uploadAttachment('019eaa00-0000-7000-8000-000000000001', 'screenshot', {
      base64: 'AAAA',
      mediaType: 'image/jpeg',
    });
    expect(seen.method).toBe('POST');
    expect(seen.url).toBe(
      'http://localhost:18080/v1/events/019eaa00-0000-7000-8000-000000000001/attachments/screenshot',
    );
    expect(seen.auth).toBe('Bearer st_pk_test');
    expect(out).not.toBeNull();
    expect(out!.ref).toBe('019e3000-7000-7000-8000-000000000001');
    expect(out!.kind).toBe('screenshot');
    expect(out!.source).toBe('js');
  });

  test('returns null on a non-201 response', async () => {
    globalThis.fetch = (async () =>
      new Response('{"error":"tooLarge"}', { status: 413 })) as typeof fetch;
    const out = await uploadAttachment('e', 'screenshot', { base64: '', mediaType: 'image/jpeg' });
    expect(out).toBeNull();
  });

  test('returns null when fetch throws (offline)', async () => {
    globalThis.fetch = (async () => {
      throw new TypeError('Network request failed');
    }) as typeof fetch;
    const out = await uploadAttachment('e', 'screenshot', { base64: '', mediaType: 'image/jpeg' });
    expect(out).toBeNull();
  });

  test('returns null without an active config (init never ran)', async () => {
    resetConfig();
    const out = await uploadAttachment('e', 'screenshot', { base64: '', mediaType: 'image/jpeg' });
    expect(out).toBeNull();
  });
});
