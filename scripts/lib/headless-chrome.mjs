// Starting a headless Chrome, once, for everything that needs one.
//
// There were two of these. `scripts/lib/web-live-driver.mjs` started
// Chrome on port 0 with `--no-first-run --no-default-browser-check`
// and read the WebSocket URL out of stderr; `webapp/devtools/sweep.mjs`
// started it on a fixed 9555 without those flags and polled
// `/json/list`. The first never failed on CI. The second failed twice
// in six runs with "chrome printed nothing at all" and the process
// still alive — the shape of a browser that came up and never opened
// its debugging port.
//
// The two were not meant to differ; they were written a week apart.
// So this is the one implementation, and `check-single-chrome-launcher`
// fails if a second one appears.
//
// What each flag is for:
//
//   --headless=new              the only headless Chrome still supported
//   --remote-debugging-port=0   the kernel picks; a fixed port can be
//                               taken, and Chrome's answer to that is
//                               to keep running with no port open,
//                               which is indistinguishable from a
//                               browser that hung
//   --user-data-dir=<mkdtemp>   a fresh profile; a locked one makes
//                               Chrome exit without printing anything
//   --no-first-run              skip the first-run machinery, which on
//                               a brand-new profile is every run
//   --no-default-browser-check  it has no business asking that here,
//                               and asking needs a desktop session
//   --disable-gpu               no GPU on a runner
//   --no-sandbox                the runner's user cannot set up the
//                               namespace sandbox
//   --disable-dev-shm-usage     a container gives /dev/shm 64 MB and
//                               the renderer dies on it silently

import { spawn } from 'node:child_process';
import { existsSync, mkdtempSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

export const chromePath = () =>
  process.env.CHROME_PATH ??
  [
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
    '/usr/bin/google-chrome',
    '/usr/bin/google-chrome-stable',
    '/usr/bin/chromium-browser',
    '/usr/bin/chromium',
  ].find((p) => existsSync(p)) ??
  'google-chrome';

/**
 * Launch, and resolve once the browser has told us where to connect.
 *
 * Returns `{ chrome, wsUrl, profile, said }`. Rejects with everything
 * worth reading rather than a bare timeout: what Chrome printed, how
 * long we waited, whether the process is still alive, and which
 * profile it was given.
 */
export async function launchChrome({ extraArgs = [], timeoutMs = 45_000 } = {}) {
  const bin = chromePath();
  const profile = mkdtempSync(join(tmpdir(), 'sentori-chrome-'));
  const chrome = spawn(
    bin,
    [
      '--headless=new',
      '--remote-debugging-port=0',
      `--user-data-dir=${profile}`,
      '--no-first-run',
      '--no-default-browser-check',
      '--disable-gpu',
      '--no-sandbox',
      '--disable-dev-shm-usage',
      ...extraArgs,
      'about:blank',
    ],
    { stdio: ['ignore', 'pipe', 'pipe'] },
  );

  let said = '';
  chrome.stdout?.on('data', (b) => (said += b.toString()));
  chrome.stderr?.on('data', (b) => (said += b.toString()));
  chrome.on('error', (e) => (said += `spawn failed: ${e.message}\n`));

  const startedAt = Date.now();
  const wsUrl = await new Promise((resolve, reject) => {
    const tick = setInterval(() => {
      const m = /ws:\/\/[^\s]+/.exec(said);
      if (m) {
        clearInterval(tick);
        resolve(m[0]);
        return;
      }
      if (Date.now() - startedAt < timeoutMs) return;
      clearInterval(tick);
      reject(
        new Error(
          `chrome never printed a debugger URL (${bin})\n` +
            `  waited ${Math.round((Date.now() - startedAt) / 1000)}s\n` +
            `  process: ${chrome.exitCode === null ? 'still running' : `exited ${chrome.exitCode}`}` +
            `${chrome.signalCode ? ` (signal ${chrome.signalCode})` : ''}\n` +
            `  profile: ${profile}\n` +
            (said.trim()
              ? `\n── what chrome said ──\n${said.trim()}\n`
              : '\n  chrome printed nothing at all — it did not get far enough to say anything.\n'),
        ),
      );
    }, 100);
  });

  return { chrome, wsUrl, profile, said: () => said };
}

/**
 * The WebSocket of a page target, for callers that drive one page
 * directly rather than through `Target.attachToTarget`.
 *
 * The browser endpoint `launchChrome` returns does not take
 * `Page.navigate`. The HTTP port comes out of that URL rather than
 * being passed in, because the whole point of asking for port 0 is
 * that nobody knows it in advance.
 */
export async function pageWebSocketUrl(browserWsUrl, { timeoutMs = 20_000 } = {}) {
  const { port } = new URL(browserWsUrl.replace(/^ws:/, 'http:'));
  const startedAt = Date.now();
  let lastSeen = null;
  for (;;) {
    try {
      const targets = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      lastSeen = targets.map((t) => t.type).join(', ') || '(empty)';
      const page = targets.find((t) => t.type === 'page');
      if (page) return page.webSocketDebuggerUrl;
    } catch {
      // The port answered a moment ago; it is coming up.
    }
    if (Date.now() - startedAt > timeoutMs) {
      throw new Error(
        `chrome opened its debugging port but never registered a page target\n` +
          `  the last /json/list held: ${lastSeen ?? '(never answered)'}`,
      );
    }
    await new Promise((r) => setTimeout(r, 200));
  }
}
