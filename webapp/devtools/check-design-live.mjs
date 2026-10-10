// Invoked by self-hosted/tests/e2e/instance.py against its disposable database.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
import { launchChrome } from '../../scripts/lib/headless-chrome.mjs';

const api = process.env.SENTORI_TEST_API;
assert.ok(api && new URL(api).hostname === '127.0.0.1' && process.env.SENTORI_TEST_PASSWORD, 'Disposable local instance required');
const { chromium, webkit } = await import(process.env.VIS_PLAYWRIGHT_MODULE ?? 'playwright-core');
const root = fileURLToPath(new URL('../', import.meta.url));
const out = fileURLToPath(new URL('../../output/v5-validation/design/', import.meta.url));
mkdirSync(out, { recursive: true });
const proxy = Object.fromEntries(['/auth', '/admin/api', '/v1', '/healthz'].map(path => [path, { target: api, changeOrigin: true }]));
const server = await createServer({ root, configFile: root + 'vite.config.ts', envDir: '/tmp/sentori-v5-no-env', server: { host: '127.0.0.1', port: 13003, strictPort: true, proxy } });
await server.listen();
const origin = 'http://127.0.0.1:13003';
const results = [];
try {
  for (const engine of ['chromium', 'webkit']) {
    let browser, running;
    try {
      if (engine === 'chromium') { running = await launchChrome(); browser = await chromium.connectOverCDP(running.wsUrl); }
      else browser = await webkit.launch();
      const context = await browser.newContext();
      const request = async (path, data, headers) => {
        const response = await context.request.post(origin + path, { data, headers });
        assert.ok(response.ok(), `${path}: ${response.status()}`);
        return response.json();
      };
      await request('/auth/login', { email: 'owner@example.test', password: process.env.SENTORI_TEST_PASSWORD });
      const first = await request('/admin/api/projects', { name: `Design ${engine} A`, platform: 'web' });
      const second = await request('/admin/api/projects', { name: `Design ${engine} B`, platform: 'web' });
      const { token } = await request(`/admin/api/projects/${second.id}/tokens`, { name: 'Browser verification', scope: 'ingest' });
      const event = await request('/v1/events', { kind: 'trace', occurredAt: new Date().toISOString(), platform: 'web', name: `Live route ${engine}` }, { Authorization: `Bearer ${token}` });
      assert.ok(event.issueId);
      const page = await context.newPage(), errors = [];
      page.on('pageerror', error => errors.push(error.message));
      for (const locale of ['zh', 'ja', 'en']) for (const theme of ['light', 'dark']) {
        await page.goto(origin + '/login');
        await page.evaluate(({ locale, theme, project }) => {
          localStorage.setItem('sentori_locale', locale);
          localStorage.setItem('sentori-theme', theme);
          localStorage.setItem('sentori_project', project);
        }, { locale, theme, project: first.id });
        for (const width of [320, 768, 1440, 1920, 2560, 3840]) {
          await page.setViewportSize({ width, height: 1000 });
          await page.goto(origin + `/settings/instance?project=${second.id}`);
          await page.locator('main input').waitFor();
          await page.waitForFunction(() => !!document.querySelector('.studio-shell'));
          const geometry = await page.evaluate(() => {
            const input = document.querySelector('main input');
            const button = document.querySelector('main .button');
            const style = getComputedStyle(document.documentElement);
            return {
              overflow: document.documentElement.scrollWidth > innerWidth + 1,
              input: input.getBoundingClientRect().height,
              button: button.getBoundingClientRect().height,
              theme: document.documentElement.dataset.theme,
              token: style.getPropertyValue('--ui-canvas').trim(),
              lang: document.documentElement.lang,
              mainHeight: document.querySelector('main').getBoundingClientRect().height,
            };
          });
          assert.equal(geometry.overflow, false);
          assert.equal(geometry.theme, theme);
          assert.equal(geometry.lang, locale);
          assert.equal(geometry.token, theme === 'dark' ? '#0d0e10' : '#f5f7fb');
          assert.equal(geometry.input, width < 640 ? 44 : 40);
          assert.ok(geometry.button >= 32 && geometry.mainHeight > 500);
          if (locale === 'zh' && theme === 'light' && [320, 1440].includes(width)) await page.screenshot({ path: `${out}${engine}-${width}.png` });
          results.push({ engine, locale, theme, width, geometry });
        }
      }
      // A saved v4 settings link retains unrelated query parameters on tab changes.
      await page.goto(origin + `/settings?tab=instance&project=${second.id}&keep=1`);
      await page.locator('main input').waitFor();
      await page.getByRole('button', { name: 'Account', exact: true }).click();
      assert.equal(new URL(page.url()).searchParams.get('keep'), '1');
      assert.equal(new URL(page.url()).searchParams.get('project'), second.id);
      // A v4 issue link must override a different persisted project and retain filters.
      await page.goto(origin + `/issues/${event.issueId}?status=open&kind=trace`);
      await page.waitForURL(url => url.searchParams.get('project') === second.id);
      assert.equal(new URL(page.url()).searchParams.get('kind'), 'trace');
      await page.locator(`.studio-nav a[href^='/releases']`).click();
      assert.equal(new URL(page.url()).searchParams.get('project'), second.id);
      assert.deepEqual(errors, []);
      await context.close();
      console.log(`PASS ${engine}: 36 live layouts, settings compatibility, legacy issue scope and navigation`);
    } finally {
      await browser?.close();
      running?.chrome.kill('SIGTERM');
      running?.chrome.stderr?.destroy();
      running?.chrome.stdout?.destroy();
      running?.chrome.unref();
    }
  }
  writeFileSync(out + 'results.json', JSON.stringify(results, null, 2) + '\n');
} finally { await server.close(); }
