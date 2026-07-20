import { afterEach, beforeEach, describe, expect, mock, test } from 'bun:test';

import { symbolicateErrorViaMetro, symbolicateStackViaMetro } from '../handlers/dev-symbolicate';
import type { Frame, SentoriError } from '../types';

const URL = 'http://localhost:8081/symbolicate';

const minified = (col: number): Frame => ({
  column: col,
  file: 'http://localhost:8081/index.bundle?platform=ios&dev=true',
  function: 'anonymous',
  inApp: false,
  line: 1,
});

// What Metro's /symbolicate sends back, positionally aligned to input.
const metroReply = (frames: { file: string; line: number; col: number; fn: string; collapse?: boolean }[]) =>
  JSON.stringify({
    stack: frames.map((f) => ({
      collapse: f.collapse ?? false,
      column: f.col,
      file: f.file,
      lineNumber: f.line,
      methodName: f.fn,
    })),
  });

const origFetch = globalThis.fetch;
afterEach(() => {
  globalThis.fetch = origFetch;
});
beforeEach(() => {
  globalThis.fetch = origFetch;
});

describe('symbolicateStackViaMetro', () => {
  test('returns null with no URL (not running from a Metro dev server)', async () => {
    // In bun:test `require("react-native")` throws → metroSymbolicateUrl() → null
    expect(await symbolicateStackViaMetro([minified(10)])).toBeNull();
  });

  test('returns null for an empty stack', async () => {
    expect(await symbolicateStackViaMetro([], { url: URL })).toBeNull();
  });

  test('maps Metro frames back to SDK frames', async () => {
    const calls: { body: unknown; url: string }[] = [];
    globalThis.fetch = (async (url: Request | string | URL, init?: RequestInit) => {
      calls.push({ body: JSON.parse((init?.body as string) ?? '{}'), url: String(url) });
      return new Response(
        metroReply([
          { col: 18, file: '/proj/src/screens/Checkout.tsx', fn: 'handleSubmit', line: 142 },
          { col: 4, collapse: true, file: '/proj/node_modules/react-native/Libraries/x.js', fn: 'r', line: 9 },
        ]),
        { headers: { 'content-type': 'application/json' }, status: 200 },
      );
    }) as typeof fetch;

    const out = await symbolicateStackViaMetro([minified(100), minified(200)], { url: URL });
    expect(calls).toHaveLength(1);
    expect(calls[0]?.url).toBe(URL);
    // request used Metro's frame shape
    expect((calls[0]?.body as { stack: { lineNumber: number }[] }).stack[0]?.lineNumber).toBe(1);

    expect(out).not.toBeNull();
    expect(out![0]).toMatchObject({
      column: 18,
      file: '/proj/src/screens/Checkout.tsx',
      function: 'handleSubmit',
      inApp: true,
      line: 142,
    });
    // node_modules + collapse → not in-app
    expect(out![1]?.inApp).toBe(false);
  });

  test('keeps the original frame when Metro can’t resolve it (file null)', async () => {
    globalThis.fetch = (async () =>
      new Response(JSON.stringify({ stack: [{ column: null, file: null, lineNumber: null, methodName: null }] }), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      })) as typeof fetch;
    const input = minified(42);
    const out = await symbolicateStackViaMetro([input], { url: URL });
    expect(out![0]).toEqual(input);
  });

  test('returns null on a non-2xx response', async () => {
    globalThis.fetch = (async () => new Response('nope', { status: 500 })) as typeof fetch;
    expect(await symbolicateStackViaMetro([minified(1)], { url: URL })).toBeNull();
  });

  test('returns null when fetch throws (Metro down)', async () => {
    globalThis.fetch = (async () => {
      throw new TypeError('ECONNREFUSED');
    }) as typeof fetch;
    expect(await symbolicateStackViaMetro([minified(1)], { url: URL })).toBeNull();
  });

  test('returns null when the reply length doesn’t match', async () => {
    globalThis.fetch = (async () =>
      new Response(metroReply([{ col: 1, file: '/a.ts', fn: 'a', line: 2 }]), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      })) as typeof fetch;
    expect(await symbolicateStackViaMetro([minified(1), minified(2)], { url: URL })).toBeNull();
  });
});

describe('metroSymbolicateUrl resolution (RN 0.83 new-arch regression)', () => {
  // Without `opts.url`, the function must resolve a real Metro URL.
  // RN 0.83 + new architecture leaves `NativeModules.SourceCode.scriptURL`
  // undefined; the fix is to prefer RN's own `getDevServer()` helper,
  // which internally calls `NativeSourceCode.getConstants().scriptURL`
  // and works on both old and new arch.
  test('prefers getDevServer() when available (works on new arch)', async () => {
    mock.module('react-native/Libraries/Core/Devtools/getDevServer', () => ({
      default: () => ({ bundleLoadedFromServer: true, url: 'http://192.168.1.100:8081/' }),
    }));
    const calls: string[] = [];
    globalThis.fetch = (async (url: Request | string | URL) => {
      calls.push(String(url));
      return new Response(metroReply([{ col: 1, file: '/proj/src/a.ts', fn: 'a', line: 5 }]), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      });
    }) as typeof fetch;

    const out = await symbolicateStackViaMetro([minified(1)]);
    expect(calls[0]).toBe('http://192.168.1.100:8081/symbolicate');
    expect(out![0]?.file).toBe('/proj/src/a.ts');
  });

  test('returns null when getDevServer says bundle was not loaded from Metro', async () => {
    mock.module('react-native/Libraries/Core/Devtools/getDevServer', () => ({
      default: () => ({ bundleLoadedFromServer: false, url: 'http://localhost:8081/' }),
    }));
    // No fallback chain hit either (NativeModules require still throws in bun env)
    expect(await symbolicateStackViaMetro([minified(1)])).toBeNull();
  });
});

describe('symbolicateErrorViaMetro', () => {
  test('replaces stack in place and recurses into the cause chain', async () => {
    globalThis.fetch = (async () =>
      new Response(metroReply([{ col: 1, file: '/proj/src/a.ts', fn: 'a', line: 5 }]), {
        headers: { 'content-type': 'application/json' },
        status: 200,
      })) as typeof fetch;

    const err: SentoriError = {
      cause: { cause: null, message: 'root', stack: [minified(2)], type: 'Error' },
      message: 'boom',
      stack: [minified(1)],
      type: 'TypeError',
    };
    await symbolicateErrorViaMetro(err, { url: URL });
    expect(err.stack[0]?.file).toBe('/proj/src/a.ts');
    expect(err.stack[0]?.line).toBe(5);
    expect(err.cause?.stack[0]?.file).toBe('/proj/src/a.ts');
  });

  test('leaves the error untouched when symbolication isn’t possible', async () => {
    globalThis.fetch = (async () => new Response('x', { status: 404 })) as typeof fetch;
    const err: SentoriError = { cause: null, message: 'boom', stack: [minified(7)], type: 'Error' };
    const before = JSON.stringify(err);
    await symbolicateErrorViaMetro(err, { url: URL });
    expect(JSON.stringify(err)).toBe(before);
  });
});
