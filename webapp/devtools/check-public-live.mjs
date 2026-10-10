// Public design and real authentication, against the disposable instance fixture.
import assert from 'node:assert/strict';
import { mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { createServer } from 'vite';
import { launchChrome } from '../../scripts/lib/headless-chrome.mjs';
const api = process.env.SENTORI_TEST_API;
assert.ok(api && new URL(api).hostname === '127.0.0.1' && process.env.SENTORI_TEST_PASSWORD);
const { chromium, webkit } = await import(process.env.VIS_PLAYWRIGHT_MODULE ?? 'playwright-core');
const root = fileURLToPath(new URL('../', import.meta.url));
const out = fileURLToPath(new URL('../../output/v5-validation/public/', import.meta.url));
mkdirSync(out, { recursive: true });
const proxy = Object.fromEntries(['/auth', '/admin/api', '/v1', '/healthz'].map(path => [path, { target: api, changeOrigin: true }]));
const server = await createServer({ root, configFile: root + 'vite.config.ts', envDir: '/tmp/sentori-v5-no-env', server: { host: '127.0.0.1', port: 13003, strictPort: true, proxy } });
await server.listen();
const origin = 'http://127.0.0.1:13003', results = [];
try {
 for (const engine of ['chromium', 'webkit']) {
  let browser, running;
  try {
   if (engine === 'chromium') { running = await launchChrome(); browser = await chromium.connectOverCDP(running.wsUrl); }
   else browser = await webkit.launch();
   const context = await browser.newContext(), page = await context.newPage(), errors = [];
   page.on('pageerror', error => errors.push(error.message));
   for (const locale of (process.env.SITE_LOCALES?.split(',') ?? ['zh', 'ja', 'en'])) for (const theme of ['light', 'dark']) {
    await page.goto(origin + '/login');
    await page.evaluate(({locale, theme}) => { localStorage.setItem('sentori_locale', locale); localStorage.setItem('sentori-theme', theme); }, {locale, theme});
    for (const width of (process.env.SITE_WIDTHS?.split(',').map(Number) ?? [320, 768, 1440, 1920, 2560, 3840])) {
     await page.setViewportSize({ width, height: 1000 });
     for (const route of ['welcome', 'demo', 'login', 'forgot-password', 'reset-password']) {
      await page.goto(origin + '/' + route); await page.locator('main h1').waitFor();
      await page.evaluate(() => document.fonts.ready);
      const state = await page.evaluate(async () => {
       document.querySelectorAll('img').forEach(img => { img.loading = 'eager'; });
       await Promise.all([...document.images].map(img => img.decode().catch(() => {})));
       return { overflow: document.documentElement.scrollWidth > innerWidth + 1, images: [...document.images].filter(i => !i.complete || !i.naturalWidth).map(i => i.src), missingCopy: document.body.innerText.includes('⟦'), lang: document.documentElement.lang };
      });
      const key = `${engine}/${locale}/${theme}/${width}/${route}`;
      assert.ok(!state.overflow, `${key} overflow`); assert.deepEqual(state.images, [], key); assert.ok(!state.missingCopy, key); assert.ok(state.lang.startsWith(locale), key);
      if (route === 'demo') {
       const tabs = page.locator('[role=tab]'); const story = page.locator('.product-story-surface'); const height = (await story.boundingBox()).height;
       await tabs.first().focus(); await page.keyboard.press('End'); assert.equal(await tabs.last().getAttribute('aria-selected'), 'true');
       assert.ok(Math.abs((await story.boundingBox()).height - height) <= 1, key + ' changing story height');
       await page.keyboard.press('Home'); assert.equal(await tabs.first().getAttribute('aria-selected'), 'true');
      }
      if (width === 320) {
       const menu = page.locator('.product-mobile-menu'); await menu.locator('summary').click(); assert.equal(await menu.getAttribute('open'), '');
       await page.keyboard.press('Escape'); assert.equal(await menu.getAttribute('open'), null);
      }
      if (locale === 'zh' && theme === 'light' && [320,1440].includes(width) && ['welcome','login'].includes(route)) await page.screenshot({ path: out + `${engine}-${width}-${route}.png`, fullPage: true });
      results.push({key, ...state});
     }
     console.log('PASS', engine, locale, theme, width);
    }
   }
   await page.goto(origin + '/login');
   await page.locator('input[type=email]').fill('owner@example.test'); await page.locator('input[type=password]').fill('incorrect-password');
   await page.locator('button[type=submit]').click(); await page.locator('[role=alert]').waitFor(); assert.ok(page.url().endsWith('/login'));
   await page.evaluate(() => sessionStorage.setItem('sentori_return_to', '/projects?from=login'));
   await page.locator('input[type=password]').fill(process.env.SENTORI_TEST_PASSWORD);
   await page.locator('button[type=submit]').click(); await page.waitForURL('**/projects?from=login');
   // Let the authenticated shell finish its bootstrap before a hard navigation.
   // WebKit reports the intentionally aborted in-flight /auth/me as a page error.
   await page.locator('.studio-shell').waitFor();
   assert.equal(await page.evaluate(() => sessionStorage.getItem('sentori_return_to')), null);
   await page.goto(origin + '/reset-password'); assert.equal(await page.locator('button[type=submit]').isDisabled(), true);
   await page.goto(origin + '/forgot-password'); await page.locator('input[type=email]').fill('owner@example.test'); await page.locator('button[type=submit]').click(); await page.locator('[role=status]').waitFor();
   await page.goto(origin + '/reset-password?token=invalid-test-token'); await page.locator('input[type=password]').first().fill('replacement-password-123'); await page.locator('input[type=password]').last().fill('replacement-password-123'); await page.locator('button[type=submit]').click(); await page.locator('[role=alert]').waitFor();
   assert.deepEqual(errors, []); console.log('PASS', engine, 'real login failure / success / return route / missing reset token');
   await context.close();
  } finally { await browser?.close(); running?.chrome.kill(); running?.chrome.stderr?.destroy(); running?.chrome.stdout?.destroy(); running?.chrome.unref(); }
 }
 writeFileSync(out + 'results.json', JSON.stringify({results}, null, 2));
 console.log(`PASS ${results.length} public page configurations`);
} finally { await server.close(); }
