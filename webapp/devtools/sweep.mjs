// Screenshot every route in one Chrome, over CDP.
//
// Spawning a browser per route costs ~4s of startup each and trips the
// "allocator loaded twice" guard when they overlap; one instance that
// navigates is both faster and the only way this finishes in one go.
//
// Also collects console errors per route — a page that renders but logs
// a throw looks fine in a screenshot, and that is exactly how the
// RangeError survived a sweep.
import { spawn } from 'node:child_process';
import { writeFileSync, mkdirSync, existsSync } from 'node:fs';

// macOS keeps Chrome in a bundle; a Linux runner has it on PATH under
// one of several names. Resolved rather than hardcoded so the same
// sweep runs on a laptop and in CI — a gate that only exists on one
// machine is a gate whoever is not at that machine does not have.
const CHROME =
  process.env.CHROME_PATH ??
  ['/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
   '/usr/bin/google-chrome', '/usr/bin/google-chrome-stable',
   '/usr/bin/chromium-browser', '/usr/bin/chromium']
    .find(p => existsSync(p)) ??
  'google-chrome';
const BASE = 'http://localhost:5599';
const I = '019f85ee-ae41-77f1-bbf9-97d310663c9a';

const ROUTES = [
  '', `issues/${I}`, 'instruments', 'releases', 'projects',
  // Settings sections are URL-driven, so each one is reachable here.
  // They were not, and four admin screens went unrendered for months.
  'settings?tab=tokens', 'settings?tab=users',
  'settings?tab=notifications',
  // Push is its own module now. Its sections are URL-driven for the
  // same reason the settings ones are — a screen the sweep cannot
  // reach is a screen nobody photographs until a user complains.
  'push', 'push?tab=audience', 'push?tab=devices', 'push?tab=credentials',
  'push?tab=integrate',
  // Seeded from the issue page — the row arrives filled in, so the
  // screenshot shows what someone actually lands on.
  `push?tab=audience&issue=${I}`,
  'settings?tab=audit', 'settings?tab=account',
  'login', 'forgot-password',
];

/** Per-route script run after load, for state behind a click. */
const OPEN_ALL = {
  // An empty condition editor photographs as an empty condition
  // editor. The part worth looking at is a counted audience next to
  // the devices it picked, which is two fields and a click away.
  'push?tab=audience': `
    (() => {
      const set = (el, v) => {
        const p = Object.getPrototypeOf(el);
        Object.getOwnPropertyDescriptor(p, 'value').set.call(el, v);
        el.dispatchEvent(new Event('input', { bubbles: true }));
      };
      // Scoped to the editor. The first \`input\` on the page is the
      // global search box, and typing into that opened the command
      // palette over the thing being photographed.
      const editor = document.querySelector('[data-audience-editor]');
      const inputs = editor ? [...editor.querySelectorAll('input')] : [];
      if (inputs[0]) set(inputs[0], 'plan');
      if (inputs[1]) set(inputs[1], 'pro');
      const count = [...document.querySelectorAll('button')]
        .find(b => !b.disabled && /\\d|[A-Za-z\\u4e00-\\u9fff]/.test(b.textContent || '') &&
                   b.closest('[data-audience-panel]'));
      if (count) count.click();
    })()
  `,
  releases:
    "[...document.querySelectorAll('button[aria-expanded=\"false\"]')].forEach(b => b.click())",
};

const out = process.argv[2] || 'tmp/sweep';
const lang = process.argv[3] || 'zh-CN';
const theme = process.argv[4] || 'dark';
// Width matters as much as language: the two-column split, the
// toolbar filters and the settings tab strip all reflow, and a
// dashboard only ever looked at on one monitor is a dashboard whose
// breakpoints nobody has seen.
const width = process.argv[5] || '1500';
mkdirSync(out, { recursive: true });

// `--disable-dev-shm-usage`: a containerised runner gives /dev/shm
// 64MB, and Chrome's renderer dies on it without a word. `stdio` is
// captured rather than discarded because when the port never opens,
// the reason is on Chrome's stderr and this script used to throw it
// away — "chrome never opened a debugging port" was every failure,
// whatever the cause.
const chrome = spawn(CHROME, [
  '--headless=new', '--disable-gpu', '--no-sandbox', '--disable-dev-shm-usage',
  '--remote-debugging-port=9555',
  `--lang=${lang}`, `--accept-lang=${lang}`,
  `--window-size=${width},1000`, `--user-data-dir=/tmp/cd-sweep-${lang}-${theme}-${width}`,
  'about:blank',
], { stdio: ['ignore', 'pipe', 'pipe'] });

let chromeSaid = '';
chrome.stdout?.on('data', (b) => { chromeSaid += b.toString(); });
chrome.stderr?.on('data', (b) => { chromeSaid += b.toString(); });
chrome.on('error', (e) => { chromeSaid += `spawn failed: ${e.message}\n`; });

// Chrome's debugging port comes up a beat after the process does, and
// on a cold CI runner that beat is longer than a laptop's. Poll for it
// — a fixed sleep here failed as `list.find(...) of undefined`, which
// reads like a bug in the sweep rather than "the browser is not up".
let list = null;
for (let i = 0; i < 40 && !list; i++) {
  try {
    const r = await fetch('http://127.0.0.1:9555/json/list');
    const j = await r.json();
    if (j.some(t => t.type === 'page')) list = j;
  } catch {
    /* not listening yet */
  }
  if (!list) await new Promise(r => setTimeout(r, 500));
}
if (!list) {
  process.stderr.write(`chrome never opened a debugging port (${CHROME})\n`);
  process.stderr.write(
    chromeSaid.trim()
      ? `\n── what chrome said ──\n${chromeSaid.trim()}\n`
      : '\nchrome printed nothing at all — it may not have started.\n',
  );
  process.stderr.write(`exited: ${chrome.exitCode ?? 'still running'}\n`);
  chrome.kill();
  process.exit(1);
}
const sock = new WebSocket(list.find(t => t.type === 'page').webSocketDebuggerUrl);
let id = 0;
const pend = new Map();
const logs = [];
await new Promise(r => { sock.onopen = r; });
sock.onmessage = e => {
  const m = JSON.parse(e.data);
  if (pend.has(m.id)) { pend.get(m.id)(m.result); pend.delete(m.id); return; }
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

// Theme lives in localStorage; set it once on the origin, then reload.
await cmd('Page.navigate', { url: `${BASE}/` });
await new Promise(r => setTimeout(r, 3500));
await cmd('Runtime.evaluate', {
  expression: `localStorage.setItem('sentori-theme', '${theme}')`,
});

const report = [];
for (const r of ROUTES) {
  logs.length = 0;
  await cmd('Page.navigate', { url: `${BASE}/${r}` });
  await new Promise(res => setTimeout(res, 3200));
  // Some of the page only exists after a click. A sweep that only
  // ever photographs the landing state cannot see an accordion's
  // contents, and the artifact list — where an unreadable upload is
  // named — lives inside one.
  if (OPEN_ALL[r]) {
    await cmd('Runtime.evaluate', { expression: OPEN_ALL[r] });
    await new Promise(res => setTimeout(res, 700));
  }
  const name = (r || 'triage').replace(I, 'I').replace(/[^\w.-]+/g, '-');
  const shot = await cmd('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true });
  if (shot?.data) writeFileSync(`${out}/${name}.png`, Buffer.from(shot.data, 'base64'));
  const text = await cmd('Runtime.evaluate', {
    expression: 'document.body.innerText.slice(0, 4000)', returnByValue: true,
  });
  report.push({ route: r, name, errors: [...logs], text: text?.result?.value ?? '' });
  process.stdout.write(`${logs.length ? '✗' : '·'} ${r}\n`);
}
// Stamp which bundle this describes. A report that cannot name its
// build is a report you have to take on trust — and one sweep here ran
// while dist/ was being rebuilt underneath it, so its clean result
// described a state that never existed on disk at any single moment.
const bundle = await cmd('Runtime.evaluate', {
  expression:
    "[...document.querySelectorAll('script[src]')].map(s => s.src.split('/').pop()).join(' ')",
  returnByValue: true,
});
// What the app asked the mock for and did not get. A page whose data
// call fell through to `{}` renders its empty state, and an empty
// state is indistinguishable from "nothing happened yet" in a
// screenshot — so this is a failure, not a footnote.
let unmocked = [];
try {
  const r = await fetch('http://localhost:8080/__unmocked');
  ({ unmocked } = await r.json());
} catch {
  unmocked = ['<mock not reachable — is `bun run mock` running?>'];
}
writeFileSync(
  `${out}/report.json`,
  JSON.stringify(
    {
      bundle: bundle?.result?.value ?? 'unknown',
      lang,
      theme,
      width,
      unmocked,
      routes: report,
    },
    null,
    1,
  ),
);
chrome.kill();

const broken = report.filter(r => r.errors.length);
for (const u of unmocked) process.stdout.write(`UNMOCKED ${u}\n`);
if (broken.length || unmocked.length) {
  process.stdout.write(
    `✗ ${broken.length} route(s) with console errors, ${unmocked.length} unmocked path(s)\n`,
  );
  process.exit(1);
}
process.stdout.write(`✓ ${report.length} routes clean\n`);
process.exit(0);
