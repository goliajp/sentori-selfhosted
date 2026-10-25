// Browser-only fixture adapter. Production is forbidden to import devtools.
import assert from 'node:assert/strict';
import { mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { resolve, sep } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createServer, preview } from 'vite';
import { launchChrome } from '../../scripts/lib/headless-chrome.mjs';

async function stopChrome(running) {
  if (!running) return;
  const child = running.chrome;
  const exited = timeout => new Promise(resolve => {
    if (child.exitCode !== null || child.signalCode !== null) return resolve(true);
    const done = value => { clearTimeout(timer); child.removeListener('exit', onExit); resolve(value); };
    const onExit = () => done(true);
    const timer = setTimeout(() => done(false), timeout);
    child.once('exit', onExit);
  });
  child.kill();
  if (!await exited(5000)) {
    child.kill('SIGKILL');
    assert.ok(await exited(5000), 'owned Chrome did not exit during cleanup');
  }
  child.stderr?.destroy();
  child.stdout?.destroy();
  child.unref();
  // The profile can still receive the browser's final atomic file renames.
  if (running.profile) rmSync(running.profile, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
}

export async function withLiveBrowsers(name, run) {
  const api = process.env.SENTORI_TEST_API, password = process.env.SENTORI_TEST_PASSWORD;
  assert.ok(api && new URL(api).hostname === '127.0.0.1' && password, 'Disposable local instance required');
  const engines = await import(process.env.VIS_PLAYWRIGHT_MODULE ?? 'playwright-core');
  const root = fileURLToPath(new URL('../', import.meta.url));
  const out = (process.env.SENTORI_TEST_OUTPUT
    ? resolve(process.env.SENTORI_TEST_OUTPUT)
    : fileURLToPath(new URL(`../../output/v5-validation/${name}/`, import.meta.url))) + sep;
  mkdirSync(out, { recursive: true });
  // The disposable server explicitly trusts this local proxy. Give each
  // browser scenario its own client address, while preserving one auth bucket
  // throughout that scenario; never forward a browser-supplied source header.
  let clientAddress;
  const proxy = Object.fromEntries(['/hooks', '/auth', '/admin/api', '/v1', '/healthz'].map(path => [path, {
    target: api, changeOrigin: true,
    configure(upstream) {
      upstream.on('proxyReq', request => request.setHeader('X-Forwarded-For', clientAddress));
    },
  }]));
  const production = process.env.SENTORI_TEST_PRODUCTION === '1';
  const config = { root, configFile: root + 'vite.config.ts', envDir: '/tmp/sentori-v5-no-env' };
  const options = { host: '127.0.0.1', port: 0, strictPort: true, proxy };
  const server = production
    ? await preview({ ...config, preview: options })
    : await createServer({ ...config, server: options });
  if (!production) await server.listen();
  const origin = `http://127.0.0.1:${server.httpServer.address().port}`;
  try {
    for (const [index, engine] of ['chromium', 'webkit'].entries()) {
      clientAddress = `198.19.0.${index + 1}`;
      let browser, running;
      try {
        if (engine === 'chromium') { running = await launchChrome(); browser = await engines.chromium.connectOverCDP(running.wsUrl); }
        else browser = await engines.webkit.launch();
        const context = await browser.newContext({locale: 'en-US'}), page = await context.newPage(), errors = [];
        page.on('pageerror', error => errors.push(error.message));
        const call = async (path, data, method = data === undefined ? 'GET' : 'POST', status = method === 'POST' && path.startsWith('/admin/api/') ? 201 : 200, request = context.request) => {
          const response = await request.fetch(origin + path, {method, data});
          assert.equal(response.status(), status, `${method} ${path}: ${await response.text()}`);
          return response.json();
        };
        try {
          await run({engine, browser, context, page, origin, out, password, call});
        } catch (error) {
          const diagnostic = await page.evaluate(() => ({
            url: location.href,
            documents: [...document.querySelectorAll('.workbench-document')].map(el => ({ id: el.id, active: el.dataset.active, hidden: el.hidden })),
            headings: [...document.querySelectorAll('h1')].map(el => el.textContent),
            alerts: [...document.querySelectorAll('[role=alert]')].map(el => el.textContent),
            activeText: document.querySelector('.workbench-document[data-active=true]')?.textContent,
          })).catch(() => ({ url: page.url() }));
          writeFileSync(out + `${engine}-failure.json`, JSON.stringify({ ...diagnostic, errors }, null, 2));
          await page.screenshot({path:out + `${engine}-failure.png`}).catch(() => {});
          throw error;
        }
        assert.deepEqual(errors, []);
        await context.close();
      } finally {
        try { await browser?.close(); }
        finally { await stopChrome(running); }
      }
    }
  } finally {
    if (production) await new Promise((resolve, reject) => server.httpServer.close(error => error ? reject(error) : resolve()));
    else await server.close();
  }
}
