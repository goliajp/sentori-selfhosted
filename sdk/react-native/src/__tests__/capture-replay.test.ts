import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';

import { __captureAndAttachReplayForTests } from '../capture';
import { setConfig, __resetForTests as resetConfig } from '../config';
import type { Event } from '../types';

// Insight 2026-05-18 rc.3 verify: walker is healthy, ticks land
// deep wireframe JSON with TextView text, but the `replay` kind
// never reaches `event.attachments`. Root cause was the inline
// `btoa(ndjson)` upload path — spec-compliant Hermes / Bun btoa
// throws `InvalidCharacterError` on any code point > 0xFF, and the
// surrounding catch swallowed the throw silently. rc.4 routes
// through `base64Utf8`, fixing both Latin-1 and UTF-8 inputs.
//
// These tests run the actual captureAndAttachReplay function the
// captureException pipeline runs. fetch is mocked at the
// transport.ts boundary so the upload landing isn't tested here
// (covered separately in screenshot.test.ts); what we care about
// is "does event.attachments grow when the input contains
// non-Latin-1 text?".

const origFetch = globalThis.fetch;

beforeEach(() => {
  setConfig({
    enabled: true,
    environment: 'test',
    errorSampleRate: null,
    ingestUrl: 'http://localhost:18080',
    release: 'app@1.0.0+1',
    screenshotsEnabled: true,
    sessionTrailEnabled: true,
    token: 'st_pk_test',
    traceSampleRate: null,
  });
});

afterEach(() => {
  globalThis.fetch = origFetch;
  resetConfig();
});

function makeEvent(): Event {
  return {
    id: '019eaa00-0000-7000-8000-000000000001',
    timestamp: '2026-05-18T05:15:24.000Z',
    kind: 'error',
    platform: 'javascript',
    release: 'app@1.0.0+1',
    environment: 'test',
    device: { os: 'android', osVersion: '36' },
    app: {
      version: '1.0.0',
      build: '1',
      framework: { name: 'react-native', version: '0.80.0' },
    },
    user: null,
    tags: undefined,
    breadcrumbs: [],
    error: { type: 'Error', message: 'boom', stack: [], cause: null },
    fingerprint: undefined,
  };
}

function mockUploadSuccess(): { calls: { url: string; body: unknown }[] } {
  const calls: { url: string; body: unknown }[] = [];
  globalThis.fetch = mock(async (url: Request | string | URL, init?: RequestInit) => {
    let body: unknown = undefined;
    try {
      body = init?.body ? JSON.parse(String(init.body)) : undefined;
    } catch {
      body = init?.body;
    }
    calls.push({ url: String(url), body });
    return new Response(
      JSON.stringify({
        kind: 'replay',
        mediaType: 'application/x-ndjson',
        refId: '019e3000-7000-7000-8000-00000000aaaa',
        sizeBytes: 200,
      }),
      { headers: { 'content-type': 'application/json' }, status: 201 },
    );
  }) as typeof fetch;
  return { calls };
}

describe('captureAndAttachReplay — Insight rc.3 regression', () => {
  test('Latin-1 ndjson attaches replay kind (control)', async () => {
    const { calls } = mockUploadSuccess();
    const event = makeEvent();
    const ndjson =
      '{"ts":1700000000,"width":390,"height":844,"nodes":[' +
      '{"kind":"text","x":10,"y":20,"w":100,"h":20,"text":"Settings"}]}';
    await __captureAndAttachReplayForTests(event, ndjson);
    expect(calls.length).toBe(1);
    expect(event.attachments?.length).toBe(1);
    expect(event.attachments?.[0]?.kind).toBe('replay');
  });

  test('UTF-8 ndjson attaches replay kind (the rc.3 Android crash repro)', async () => {
    // This is the bug: pre-rc.4 `btoa(ndjson)` threw on the
    // Japanese characters below, the catch swallowed it, the
    // attachment silently never landed. Post-rc.4 the helper
    // handles UTF-8 and the attachment lands normally.
    const { calls } = mockUploadSuccess();
    const event = makeEvent();
    const ndjson =
      '{"ts":1700000000,"width":390,"height":844,"nodes":[' +
      '{"kind":"text","x":10,"y":20,"w":100,"h":20,"text":"設定"},' +
      '{"kind":"text","x":10,"y":50,"w":100,"h":20,"text":"ログアウト"}]}';
    await __captureAndAttachReplayForTests(event, ndjson);
    expect(calls.length).toBe(1);
    expect(event.attachments?.length).toBe(1);
    expect(event.attachments?.[0]?.kind).toBe('replay');
  });

  test('mixed em-dash / smart-quote / emoji attaches replay kind', async () => {
    const { calls } = mockUploadSuccess();
    const event = makeEvent();
    const ndjson =
      '{"ts":1,"width":390,"height":844,"nodes":[' +
      '{"kind":"text","x":0,"y":0,"w":50,"h":20,"text":"hello — world"},' +
      '{"kind":"text","x":0,"y":30,"w":50,"h":20,"text":"\\"quoted\\""},' +
      '{"kind":"text","x":0,"y":60,"w":50,"h":20,"text":"🎉"}]}';
    await __captureAndAttachReplayForTests(event, ndjson);
    expect(calls.length).toBe(1);
    expect(event.attachments?.length).toBe(1);
    expect(event.attachments?.[0]?.kind).toBe('replay');
  });

  test('upload returns null → no attachment, no throw', async () => {
    globalThis.fetch = (async () =>
      new Response('{"error":"tooLarge"}', { status: 413 })) as typeof fetch;
    const event = makeEvent();
    const ndjson = '{"ts":1,"nodes":[]}';
    await __captureAndAttachReplayForTests(event, ndjson);
    expect(event.attachments).toBeUndefined();
  });

  test('fetch throws (offline) → no attachment, no throw out of the function', async () => {
    globalThis.fetch = (async () => {
      throw new TypeError('Network request failed');
    }) as typeof fetch;
    const event = makeEvent();
    const ndjson = '{"ts":1,"nodes":[]}';
    await __captureAndAttachReplayForTests(event, ndjson);
    expect(event.attachments).toBeUndefined();
  });
});
