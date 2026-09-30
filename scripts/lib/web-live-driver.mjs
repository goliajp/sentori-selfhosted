// Drive the browser SDK in a real Chrome, over CDP.
//
//   node scripts/lib/web-live-driver.mjs <pageUrl> <outFile>
//
// Everything the web SDK does that matters happens against APIs Bun
// does not have: `addEventListener('error')`, `PerformanceObserver`,
// `localStorage` in a private window, `visibilitychange`. Unit tests
// of those would be tests of whatever stand-in was written for them.
//
// So: a real page, a real uncaught error, a real click, a real
// rejection — and the long-task budget measured in the same run,
// because a reporter that costs the page a frame is one the host
// removes.

import { writeFileSync } from 'node:fs';

import { launchChrome } from './headless-chrome.mjs';

const [pageUrl, outFile] = process.argv.slice(2);
if (!pageUrl || !outFile) {
  console.error('usage: web-live-driver.mjs <pageUrl> <outFile>');
  process.exit(1);
}

// One launcher, shared with the render sweep. They were two, and the
// pair that differed by `--no-first-run` and a fixed port is the pair
// where one failed on CI and the other never did.
const { chrome, wsUrl } = await launchChrome().catch((e) => {
  console.error(e.message);
  process.exit(1);
});

const ws = new WebSocket(wsUrl);
await new Promise((r) => ws.addEventListener('open', r, { once: true }));

let nextId = 1;
const waiting = new Map();
ws.addEventListener('message', (m) => {
  const msg = JSON.parse(m.data);
  if (msg.id && waiting.has(msg.id)) {
    const { resolve, reject } = waiting.get(msg.id);
    waiting.delete(msg.id);
    msg.error ? reject(new Error(JSON.stringify(msg.error))) : resolve(msg.result);
  }
});
const send = (method, params = {}, sessionId) =>
  new Promise((resolve, reject) => {
    const id = nextId++;
    waiting.set(id, { resolve, reject });
    ws.send(JSON.stringify({ id, method, params, sessionId }));
  });

const { targetId } = await send('Target.createTarget', { url: 'about:blank' });
const { sessionId } = await send('Target.attachToTarget', { targetId, flatten: true });
const call = (method, params) => send(method, params, sessionId);
await call('Page.enable');
await call('Runtime.enable');

const evaluate = async (expression, awaitPromise = false) => {
  const r = await call('Runtime.evaluate', {
    expression,
    awaitPromise,
    returnByValue: true,
  });
  if (r.exceptionDetails) {
    throw new Error(`page threw: ${r.exceptionDetails.text} ${JSON.stringify(r.result?.value ?? '')}`);
  }
  return r.result.value;
};

const fail = async (m) => {
  console.error(`✗ ${m}`);
  chrome.kill();
  process.exit(1);
};

await call('Page.navigate', { url: pageUrl });
// The module has to load and `init` has to run.
for (let i = 0; i < 100; i += 1) {
  if (await evaluate('!!window.__ready')) break;
  await new Promise((r) => setTimeout(r, 100));
}
if (!(await evaluate('!!window.__ready'))) {
  await fail('the harness page never initialised the SDK');
}

// ── first what the person did, then what broke ───────────────────
// In that order, because the ring is snapshotted when the event is
// built: an error thrown before the click would carry a ring without
// it, and asserting on that would be asserting the wrong thing. It is
// also the real sequence — someone clicks, and then it breaks.
await evaluate(`
  document.getElementById('pay').click();
  history.pushState({}, '', '/checkout');
  true;
`);
// Long enough for the replay ring to hold more than one frame at 2 Hz.
await new Promise((r) => setTimeout(r, 1400));

await evaluate(`
  window.__hostSawIt = false;
  window.addEventListener('error', () => { window.__hostSawIt = true; });
  setTimeout(() => { throw new TypeError('uncaught from the page'); }, 0);
  true;
`);
await new Promise((r) => setTimeout(r, 200));

// The host's own listener must still fire. Installing ours by
// assigning window.onerror would have replaced theirs, and the
// symptom is "our error reporting stopped when we added Sentori".
if (!(await evaluate('window.__hostSawIt'))) {
  await fail("the page's own error listener stopped firing — we replaced it instead of joining it");
}

// ── an unhandled rejection ────────────────────────────────────────
await evaluate(`Promise.reject(new RangeError('nobody caught this')); true;`);
await new Promise((r) => setTimeout(r, 200));

// ── the cost of using the SDK, measured on this page ─────────────
//
// The first version of this put 550 calls in one synchronous loop and
// asserted no `longtask`. That measures the loop, not the SDK: a
// single task doing 550 of anything is a long task on a slow machine
// by definition, and it went red on the CI runner at 83 and 120 ms
// while passing locally. No page calls an error reporter that way.
//
// So the calls are spread the way a page makes them — a handful per
// task — and the assertion is the real one: using the SDK does not
// produce a long task. The per-call cost is measured too and printed,
// because a number nobody looks at is how a budget rots.
const cost = await evaluate(`
  new Promise((resolve) => {
    const seen = [];
    let observing = false;
    try {
      const o = new PerformanceObserver((l) => {
        for (const e of l.getEntries()) seen.push(Math.round(e.duration));
      });
      o.observe({ type: 'longtask' });
      observing = true;
    } catch { /* an engine without longtask; the totals still mean something */ }

    const CHUNKS = 55;
    const PER_CHUNK = 10;
    let done = 0;
    let sdkMs = 0;
    const step = () => {
      const t0 = performance.now();
      for (let i = 0; i < PER_CHUNK; i += 1) {
        if (done % 11 === 0) window.__sentori.error(new Error('load ' + done));
        else window.__sentori.trace('tick', { i: done });
        done += 1;
      }
      sdkMs += performance.now() - t0;
      if (done < CHUNKS * PER_CHUNK) setTimeout(step, 0);
      else setTimeout(() => resolve({ longTasks: seen, sdkMs, calls: done, observing }), 300);
    };
    step();
  });
`, true);
const longTasks = cost.longTasks;

await evaluate('window.__sentori.flush()', true);
// The batch is fire-and-forget inside the transport; give it a beat.
await new Promise((r) => setTimeout(r, 800));
await evaluate('window.__sentori.flush()', true);
await new Promise((r) => setTimeout(r, 800));

if (!cost.observing) {
  await fail('this browser has no `longtask` entry type, so the budget was never measured');
}
writeFileSync(outFile, JSON.stringify(cost));
console.log(`  a real page: uncaught error, rejection, click, navigation`);
console.log(
  `  ${cost.calls} SDK calls spread over tasks: ${cost.sdkMs.toFixed(1)} ms of SDK time ` +
    `(${((cost.sdkMs / cost.calls) * 1000).toFixed(0)} µs each), ` +
    `long tasks: ${longTasks.length === 0 ? 'none' : longTasks.join(', ') + ' ms'}`,
);

chrome.kill();
