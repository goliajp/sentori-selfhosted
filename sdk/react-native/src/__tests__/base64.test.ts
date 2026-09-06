import { describe, expect, test } from 'bun:test';

import { base64Utf8 } from '../base64';

// Bun ships a spec-compliant `globalThis.btoa` (Latin-1 only — throws
// on code points > 0xFF). That makes bun:test a faithful stand-in
// for Hermes' behaviour, which is what runs in the RN app at runtime.

describe('base64Utf8', () => {
  test('ASCII round-trips identically to plain btoa', () => {
    const s = '{"ts":1,"nodes":[]}';
    const encoded = base64Utf8(s);
    expect(encoded).toBe(globalThis.btoa(s));
    expect(globalThis.atob(encoded)).toBe(s);
  });

  test('handles Japanese text (the rc.3 Android crash repro)', () => {
    // The wireframe NDJSON the rc.3 walker produces now includes
    // deep TextView content. If any of that text has code points
    // > 0xFF, the *unsafe* `btoa(s)` path throws and the attachment
    // silently disappears. This is the bug Insight reported.
    const s = '{"kind":"text","text":"こんにちは"}';
    const encoded = base64Utf8(s);
    // Round-trip through the standard decoder back to the same
    // UTF-8 string. atob gives back the Latin-1-encoded byte
    // sequence; decodeURIComponent + escape reverses it.
    const decoded = decodeURIComponent(escape(globalThis.atob(encoded)));
    expect(decoded).toBe(s);
  });

  test('handles em-dash / smart-quote / emoji', () => {
    const s = 'label — "smart" 🎉';
    const encoded = base64Utf8(s);
    const decoded = decodeURIComponent(escape(globalThis.atob(encoded)));
    expect(decoded).toBe(s);
  });

  test('the *unsafe* btoa(s) path that the SDK used pre-fix throws on the same input', () => {
    // Locks in why we need this helper at all. If a future
    // refactor reverts to plain `btoa(s)` on UTF-8 NDJSON, the
    // call below will catch it — proving silently-broken
    // attachments aren't acceptable.
    expect(() => globalThis.btoa('こんにちは')).toThrow();
  });

  test('handles a full multi-line wireframe NDJSON with JP labels', () => {
    const ndjson = [
      '{"ts":1700000000,"width":390,"height":844,"nodes":[{"kind":"text","x":10,"y":20,"w":100,"h":20,"text":"設定"}]}',
      '{"ts":1700000001,"width":390,"height":844,"nodes":[{"kind":"text","x":10,"y":20,"w":100,"h":20,"text":"ログアウト"}]}',
    ].join('\n');
    const encoded = base64Utf8(ndjson);
    const decoded = decodeURIComponent(escape(globalThis.atob(encoded)));
    expect(decoded).toBe(ndjson);
  });
});
