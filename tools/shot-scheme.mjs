// Screenshot a page under a real emulated colour scheme.
//
// `--force-dark-mode` is Chrome's auto-darkening *filter*, not
// `prefers-color-scheme: dark` — it inverts a light page, which is how
// black SVG text screenshotted as white and a real bug survived four
// rounds of looking at pictures.
import { spawn } from 'node:child_process';
import { writeFileSync } from 'node:fs';

const [file, out, scheme, w, h] = process.argv.slice(2);
const CHROME = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const port = 9722;
const chrome = spawn(CHROME, ['--headless=new', '--disable-gpu', '--no-sandbox',
  `--remote-debugging-port=${port}`, '--hide-scrollbars',
  `--window-size=${w},${h}`, `--user-data-dir=/tmp/cd-shot-${scheme}`, 'about:blank'],
  { stdio: 'ignore' });

let list = null;
for (let i = 0; i < 40 && !list; i++) {
  try {
    const j = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
    if (j.some(t => t.type === 'page')) list = j;
  } catch { /* not up */ }
  if (!list) await new Promise(r => setTimeout(r, 500));
}
const sock = new WebSocket(list.find(t => t.type === 'page').webSocketDebuggerUrl);
let id = 0; const pend = new Map();
await new Promise(r => { sock.onopen = r; });
sock.onmessage = e => { const m = JSON.parse(e.data); if (pend.has(m.id)) pend.get(m.id)(m.result); };
const cmd = (method, params = {}) => new Promise(r => { pend.set(++id, r); sock.send(JSON.stringify({ id, method, params })); });

await cmd('Page.enable');
await cmd('Emulation.setEmulatedMedia', {
  features: [{ name: 'prefers-color-scheme', value: scheme }],
});
await cmd('Page.navigate', { url: `file://${file}` });
await new Promise(r => setTimeout(r, 2500));
const shot = await cmd('Page.captureScreenshot', { format: 'png', captureBeyondViewport: true });
writeFileSync(out, Buffer.from(shot.data, 'base64'));
sock.close(); chrome.kill();
process.exit(0);
