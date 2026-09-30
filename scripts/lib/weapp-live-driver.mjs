// Drive the mini-program SDK against a real Sentori server.
//
//   node scripts/lib/weapp-live-driver.mjs <baseUrl> <token> <outFile>
//
// There is no WeChat runtime here, so `wx` is a shim: `request` is a
// real HTTP call, storage is a real in-memory store with the base
// library's 1 MB-per-key behaviour, and the five `on*` hooks are
// functions the driver calls. Everything above that — the verbs, the
// transport, the identity hashing, the event assembly — is the code
// that ships.
//
// A shim is a compromise and worth naming as one: it proves the wire,
// the queueing, the storage path and the handler wiring. It does not
// prove the SDK runs inside WeChat's engine, which needs a phone.

import { writeFileSync } from 'node:fs';

// Before the SDK is imported: a WeChat mini program has no
// `crypto.subtle`, and Node does. Leaving Node's in place would test
// the path that does not run on the target and leave the pure-JS
// fallback — the only reason identity works there at all — unexercised
// by the one gate that talks to a server.
Object.defineProperty(globalThis, 'crypto', { value: undefined, configurable: true });

const { sentori, setWxHost } = await import('../../sdk/weapp/lib/index.js');

const [base, token, outFile] = process.argv.slice(2);
if (!base || !token || !outFile) {
  console.error('usage: weapp-live-driver.mjs <baseUrl> <token> <outFile>');
  process.exit(1);
}

const fail = (m) => {
  console.error(`✗ ${m}`);
  process.exit(1);
};

// ── the shim ──────────────────────────────────────────────────────
const store = new Map();
const handlers = {};
let requestCount = 0;

const wx = {
  request(options) {
    requestCount += 1;
    fetch(options.url, {
      method: options.method ?? 'GET',
      headers: options.header ?? {},
      body: options.data,
    })
      .then(async (res) => {
        // The base library parses JSON when the response says so.
        const text = await res.text();
        let data = text;
        if ((res.headers.get('content-type') ?? '').includes('json')) {
          try {
            data = JSON.parse(text);
          } catch {
            data = text;
          }
        }
        options.success?.({ statusCode: res.status, data });
      })
      .catch((e) => options.fail?.({ errMsg: `request:fail ${e.message}` }))
      .finally(() => options.complete?.());
  },
  getStorageSync: (k) => store.get(k) ?? '',
  setStorageSync: (k, v) => {
    // The real one throws over 1 MB per key. The SDK is supposed to
    // stay under that on its own; a shim that silently accepted any
    // size would hide the day it stops.
    if (typeof v === 'string' && Buffer.byteLength(v) > 1024 * 1024) {
      throw new Error('setStorage:fail exceed max size');
    }
    store.set(k, v);
  },
  removeStorageSync: (k) => store.delete(k),
  getSystemInfoSync: () => ({
    brand: 'devtools',
    model: 'iPhone 15',
    system: 'iOS 17.0',
    platform: 'devtools',
    language: 'zh_CN',
    windowWidth: 375,
    windowHeight: 812,
    pixelRatio: 3,
    SDKVersion: '3.4.0',
  }),
  onError: (fn) => (handlers.error = fn),
  onUnhandledRejection: (fn) => (handlers.rejection = fn),
  onPageNotFound: (fn) => (handlers.pageNotFound = fn),
  onMemoryWarning: (fn) => (handlers.memoryWarning = fn),
  onLazyLoadError: (fn) => (handlers.lazyLoadError = fn),
};

setWxHost(wx);

// ── init, exactly as a mini program's app.js would ────────────────
sentori.init({
  token,
  ingestUrl: base,
  release: 'weapp@1.0.0+1',
  environment: 'test',
});
sentori.user({ id: 'weapp-person', email: null });
// `user()` is synchronous and the hash lands a tick later — by design,
// so the verb never returns a promise. A mini program signs in at
// launch and fails minutes afterwards; here the two are microseconds
// apart, so the wait is the harness matching reality rather than the
// SDK needing one.
await new Promise((r) => setTimeout(r, 50));

for (const name of ['error', 'rejection', 'pageNotFound', 'memoryWarning', 'lazyLoadError']) {
  if (typeof handlers[name] !== 'function') {
    fail(`init registered no handler for ${name} — \`wx.on${name[0].toUpperCase()}${name.slice(1)}\` was never called`);
  }
}

// ── the five things the platform reports ─────────────────────────
// `wx.onError` hands a string, not an Error: message and stack already
// flattened into one blob. Shaped exactly as the base library does.
handlers.error(
  'TypeError: Cannot read property "total" of undefined\n' +
    '    at checkout (pages/cart/index.js:48:17)\n' +
    '    at Object.tap (pages/cart/index.js:12:3)',
);
handlers.rejection({ reason: new RangeError('nobody caught this') });
handlers.pageNotFound({ path: 'pages/gone/index' });
handlers.memoryWarning({ level: 10 });
handlers.lazyLoadError({ type: 'subpackage', errMsg: 'loadSubpackage:fail' });

// ── a request the host makes, which must still work ──────────────
const hostSaw = await new Promise((resolve) => {
  wx.request({
    url: `${base}/healthz`,
    method: 'GET',
    success: (res) => resolve(res.statusCode),
    fail: () => resolve(null),
  });
});
if (hostSaw !== 200) {
  fail(`the host's own wx.request stopped working after the patch (got ${hostSaw})`);
}

await sentori.flush();
await new Promise((r) => setTimeout(r, 1200));
await sentori.flush();
await new Promise((r) => setTimeout(r, 1200));

writeFileSync(outFile, JSON.stringify({ requestCount, stored: [...store.keys()] }));
console.log(`  five platform failures reported; ${requestCount} wx.request call(s) in total`);
