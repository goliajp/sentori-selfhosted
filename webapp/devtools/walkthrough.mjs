// Walk the product the way somebody evaluating it would: a real server,
// a real login, real data — and a screenshot of every screen they would
// reach.
//
// `sweep.mjs` answers a different question. It drives a mock API so a
// rendering bug shows up without a stack behind it, which is what a
// developer wants mid-change. It cannot tell you that the empty state
// reads badly, that a number lands with no unit, or that the first
// screen after signing in says nothing about what to do next — because
// nothing it renders came from a server that had to decide what to say.
//
//   docker compose … up     (or: SENTORI_E2E_KEEP=1 smoke.sh)
//   SENTORI_EMAIL=… SENTORI_PASSWORD=… node webapp/devtools/walkthrough.mjs out/
//
// Writes <out>/<route>.png plus <out>/report.json — console errors and
// load time per route, so a screenshot that looks calm and a page that
// threw are not the same finding.
import { writeFileSync, mkdirSync, mkdtempSync } from 'node:fs';

import { launchChrome, pageWebSocketUrl } from '../../scripts/lib/headless-chrome.mjs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const BASE = process.env.SENTORI_BASE || 'http://127.0.0.1:18080';
const EMAIL = process.env.SENTORI_EMAIL;
const PASSWORD = process.env.SENTORI_PASSWORD;
const out = process.argv[2] || mkdtempSync(join(tmpdir(), 'sentori-walk-'));
const lang = process.argv[3] || 'zh-CN';
const theme = process.argv[4] || 'dark';
const width = process.argv[5] || '1500';

if (!EMAIL || !PASSWORD) {
  process.stderr.write('SENTORI_EMAIL and SENTORI_PASSWORD are required — ' +
    'this walks the product as a signed-in user, and a login page is not ' +
    'the product.\n');
  process.exit(1);
}

// Sign in over HTTP first: the cookie is what makes every later
// navigation land on a real screen instead of a redirect to /login.
const login = await fetch(`${BASE}/auth/login`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ email: EMAIL, password: PASSWORD }),
});
if (!login.ok) {
  process.stderr.write(`login failed: ${login.status} ${await login.text()}\n`);
  process.exit(1);
}
const setCookie = login.headers.get('set-cookie') || '';
const session = /([^=;,\s]+)=([^;]+)/.exec(setCookie);
if (!session) {
  process.stderr.write(`login returned no cookie: ${setCookie}\n`);
  process.exit(1);
}

const cookie = `${session[1]}=${session[2]}`;
const issues = await (await fetch(`${BASE}/admin/api/issues`, { headers: { cookie } }))
  .json().catch(() => null);
const candidates = issues?.issues ?? (Array.isArray(issues) ? issues : []);

// Not simply the newest. The issue page's main panel is the stack, and
// the newest issue is usually a probe or a validation fixture with an
// empty one — so this walked past the screen people came for and
// photographed three empty cards instead. Two review rounds read that
// as "the product cannot show a stack" when the product could and the
// *picture* could not.
//
// So: find an issue whose latest event actually carries frames, and
// fail loudly rather than settle for one that does not.
let iid = null;
let best = 0;
for (const candidate of candidates) {
  const occurrences = await (await fetch(
    `${BASE}/admin/api/issues/${candidate.id}/events`,
    { headers: { cookie } },
  )).json().catch(() => null);
  const eventId = (occurrences?.events ?? [])[0]?.id;
  if (!eventId) continue;
  const event = await (await fetch(`${BASE}/admin/api/events/${eventId}`, {
    headers: { cookie },
  })).json().catch(() => null);
  // Ranked by *readable* frames, not by frames. A stack of
  // `p.q.r.a ?:100` renders the panel and proves nothing — it is an
  // unsymbolicated twin of an issue that does resolve, and landing on
  // it photographs the product failing at the one thing it is for.
  // Two review rounds concluded "this product cannot show a stack"
  // from shots like that, while an issue resolving to
  // `src/cart/total.ts:48` sat in the same list.
  const frames = event?.payload?.error?.stack ?? [];
  const readable = frames.filter(
    (f) => f && f.file && f.file !== '?' && f.file !== '<unknown>' && f.line,
  ).length;
  if (readable > best) {
    best = readable;
    iid = candidate.id;
  }
}
if (!iid) {
  // The issue page is the product, and a shot of its empty state is
  // not a shot of it.
  process.stderr.write(
    `no issue at ${BASE}/admin/api/issues has an event whose stack resolves to ` +
      `a file and a line, so the one screen this walkthrough exists for would be ` +
      `photographed showing the product failing at its own job. Seed one first ` +
      `(smoke.sh does).\n`,
  );
  process.exit(1);
}

// What somebody evaluating this would open, in the order they would
// open it.
const ROUTES = [
  '', 'instruments', 'releases', 'projects',
  `issues/${iid}`,
  'push', 'push?tab=credentials', 'push?tab=integrate',
  'settings?tab=tokens', 'settings?tab=users', 'settings?tab=notifications',
  'settings?tab=account', 'settings?tab=audit',
];

mkdirSync(out, { recursive: true });

// The shared launcher, same as the render sweep and the web SDK's
// live-ingest driver. This was a third copy — fixed port 9556, a
// profile path keyed on language and theme, and no `--no-first-run`.
// `check-single-chrome-launcher` found it the moment that check
// existed.
let chrome;
let pageWs;
try {
  const started = await launchChrome({
    extraArgs: [`--lang=${lang}`, `--accept-lang=${lang}`, `--window-size=${width},1000`],
  });
  chrome = started.chrome;
  pageWs = await pageWebSocketUrl(started.wsUrl);
} catch (e) {
  process.stderr.write(`${e.message}\n`);
  chrome?.kill();
  process.exit(1);
}

const sock = new WebSocket(pageWs);
let id = 0;
const pend = new Map();
let logs = [];
const loadWaiters = [];
await new Promise(r => { sock.onopen = r; });
sock.onmessage = e => {
  const m = JSON.parse(e.data);
  if (pend.has(m.id)) { pend.get(m.id)(m.result); pend.delete(m.id); return; }
  if (m.method === 'Page.loadEventFired') { for (const w of [...loadWaiters]) w(); }
  if (m.method === 'Runtime.exceptionThrown') {
    logs.push(m.params?.exceptionDetails?.exception?.description ?? 'exception');
  }
  if (m.method === 'Runtime.consoleAPICalled' && m.params.type === 'error') {
    logs.push(m.params.args.map(a => a.value ?? a.description ?? '').join(' '));
  }
};
const cmd = (method, params = {}) =>
  new Promise(r => { const i = ++id; pend.set(i, r); sock.send(JSON.stringify({ id: i, method, params })); });

await cmd('Page.enable');
await cmd('Runtime.enable');
await cmd('Network.enable');
const u = new URL(BASE);
await cmd('Network.setCookie', {
  name: session[1], value: session[2], domain: u.hostname, path: '/',
});
await cmd('Page.navigate', { url: `${BASE}/` });
await new Promise(r => setTimeout(r, 2500));
await cmd('Runtime.evaluate', { expression: `localStorage.setItem('sentori-theme', '${theme}')` });

const report = [];
for (const r of ROUTES) {
  logs = [];
  // Time to the load event, not to the screenshot. The screenshot waits
  // a fixed beat so late renders land in the picture; recording that
  // wait as "ms" gave every route the same 3 s and would have been read
  // as a performance number.
  const started = Date.now();
  let loadedAt = null;
  const onLoad = () => { loadedAt = Date.now(); };
  loadWaiters.push(onLoad);
  await cmd('Page.navigate', { url: `${BASE}/${r}` });
  await new Promise(x => setTimeout(x, 3000));
  loadWaiters.splice(loadWaiters.indexOf(onLoad), 1);
  const ms = loadedAt ? loadedAt - started : null;
  const shot = await cmd('Page.captureScreenshot', { format: 'png' });
  const name = (r || 'home').replace(/[/?=&]/g, '_');
  writeFileSync(join(out, `${name}.png`), Buffer.from(shot.data, 'base64'));
  const title = await cmd('Runtime.evaluate', { expression: 'document.title', returnByValue: true });
  const text = await cmd('Runtime.evaluate', {
    expression: 'document.body.innerText.slice(0, 900)', returnByValue: true,
  });
  report.push({
    route: r || '/', file: `${name}.png`, msToLoad: ms,
    title: title?.result?.value, errors: [...logs],
    text: text?.result?.value,
  });
  process.stdout.write(`${logs.length ? '✗' : '·'} ${r || '/'} ` +
    `${ms === null ? 'no load event' : `${ms}ms`}\n`);
}

writeFileSync(join(out, 'report.json'), JSON.stringify(report, null, 2));
chrome.kill();
const broken = report.filter(r => r.errors.length);
process.stdout.write(`\n${report.length} screens in ${out}` +
  `${broken.length ? `, ${broken.length} with console errors` : ''}\n`);
