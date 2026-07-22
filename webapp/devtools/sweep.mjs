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
import { writeFileSync, mkdirSync } from 'node:fs';

const CHROME = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const BASE = 'http://localhost:5599';
const P = '019e358a-adac-7881-9f7e-fc92646fae4e';
const I = '019f85ee-ae41-77f1-bbf9-97d310663c9a';

const ROUTES = [
  'main', 'projects', 'members', 'alerts', 'audit', 'health', 'notifications',
  'saved-views', 'search', 'sessions', 'settings', 'settings/billing', 'saas',
  'shortcuts', 'login', 'register', 'forgot-password',
  `projects/${P}/issues`, `projects/${P}/issues/${I}`, `projects/${P}/events`,
  `projects/${P}/traces`, `projects/${P}/metrics`, `projects/${P}/replays`,
  `projects/${P}/tokens`, `projects/${P}/releases`, `projects/${P}/integrations`,
  `projects/${P}/cert`, `projects/${P}/probes`, `projects/${P}/push`,
  `projects/${P}/push-sends`,
];

const out = process.argv[2] || 'tmp/sweep';
const lang = process.argv[3] || 'zh-CN';
const theme = process.argv[4] || 'dark';
mkdirSync(out, { recursive: true });

const chrome = spawn(CHROME, [
  '--headless=new', '--disable-gpu', '--remote-debugging-port=9555',
  `--lang=${lang}`, `--accept-lang=${lang}`,
  '--window-size=1500,1000', `--user-data-dir=/tmp/cd-sweep-${lang}-${theme}`,
  'about:blank',
], { stdio: 'ignore' });
await new Promise(r => setTimeout(r, 3000));

const list = await (await fetch('http://127.0.0.1:9555/json/list')).json();
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
await cmd('Page.navigate', { url: `${BASE}/main` });
await new Promise(r => setTimeout(r, 3500));
await cmd('Runtime.evaluate', {
  expression: `localStorage.setItem('gds-theme', JSON.stringify({...(JSON.parse(localStorage.getItem('gds-theme')||'{}')), mode:'${theme}'}))`,
});

const report = [];
for (const r of ROUTES) {
  logs.length = 0;
  await cmd('Page.navigate', { url: `${BASE}/${r}` });
  await new Promise(res => setTimeout(res, 3200));
  const name = r.replaceAll('/', '-').replace(P, 'P').replace(I, 'I');
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
writeFileSync(
  `${out}/report.json`,
  JSON.stringify(
    { bundle: bundle?.result?.value ?? 'unknown', lang, theme, routes: report },
    null,
    1,
  ),
);
chrome.kill();
process.exit(0);
