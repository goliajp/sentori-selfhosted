import { readFile } from 'node:fs/promises';
import { chromium } from 'playwright';
import lighthouse from 'lighthouse';
import { launchChrome, stopChrome } from '../../scripts/lib/headless-chrome.mjs';
import { startProxy, safeUrl, resolveTarget } from './proxy.mjs';
import { analyzeSeo, analyzeRobots, sitemapContains } from './seo.mjs';

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const reportUrl = value => {
  const url = new URL(safeUrl(value));
  url.pathname = url.pathname.split('/').map(part => /^\d+$|^[a-f0-9-]{20,}$/i.test(part) || part.length > 64 || /%40|@/i.test(part) ? ':id' : part).join('/');
  return url.href;
};
export function authState(profiles, job) {
  const name = job.configuration.authProfile;
  if (!name) return null;
  const entry = profiles.find(p => p.projectId === job.projectId && p.appId === job.appId && p.name === name);
  if (!entry?.storageState) throw new Error('auth_profile_missing');
  const state = entry.storageState;
  const origins = new Set([new URL(job.configuration.url).origin, ...(job.configuration.allowedOrigins || [])]);
  if (!Array.isArray(state.cookies) || !Array.isArray(state.origins) || state.cookies.length > 100 || state.origins.length > 16) throw new Error('auth_profile_invalid');
  if (state.origins.some(row => !origins.has(row.origin)) || state.cookies.some(cookie => ![...origins].some(origin => {
    const host = new URL(origin).hostname;
    const domain = cookie.domain.replace(/^\./, '');
    return host === domain; // Do not grant a parent-domain credential to unrelated sibling hosts.
  }))) throw new Error('auth_profile_origin');
  return state;
}
export async function check(job, profiles = []) {
  const config = job.configuration;
  const result = {
    source: 'synthetic', status: 'passed', ruleVersion: 1, checkedAt: new Date().toISOString(), url: reportUrl(config.url),
    profile: { device: config.device || 'desktop', network: config.device==='mobile'?'simulated-4G':'simulated-broadband', browserNetwork:'unthrottled', cpuSlowdown:config.device==='mobile'?'4':'1', location: process.env.SENTORI_WEB_RUNNER_LOCATION || 'unspecified' },
    versions: { runner: '1.0.0', browser: 'unavailable', lighthouse: 'not-run' }, metrics: [], findings: [],
    coverage: { lighthouse: 'not-requested', indexing: 'not-measured', pagesChecked: 0, blockedRequests: 0 },
  };
  function finding(rule, severity, evidence, advice, url = config.url) {
    if (result.findings.length < 100) result.findings.push({ rule, severity, evidence: evidence.slice(0,500), advice: advice.slice(0,500), url: reportUrl(url) });
  }
  const allowed = new Set([new URL(config.url).origin, ...(config.allowedOrigins || []).map(url => new URL(url).origin)]);
  let proxy, launched, browser, watchdog;
  try {
    const credentials = authState(profiles, job);
    await resolveTarget(config.url, allowed);
    proxy = await startProxy(allowed);
    const chromeEnv = { PATH: process.env.PATH, HOME: '/tmp', LANG: 'C.UTF-8' };
    launched = await launchChrome({ sandbox: process.env.SENTORI_WEB_RUNNER_SANDBOX !== 'off', env: chromeEnv, timeoutMs: 20_000, extraArgs: [
      `--proxy-server=${proxy.url}`, '--proxy-bypass-list=<-loopback>', '--disable-quic', '--disable-background-networking',
      '--disable-component-update', '--disable-extensions', '--disable-sync', '--force-webrtc-ip-handling-policy=disable_non_proxied_udp',
    ] });
    watchdog = setTimeout(() => launched.chrome.kill('SIGKILL'), 240_000);
    browser = await chromium.connectOverCDP(launched.wsUrl, { timeout: 15_000 });
    result.versions.browser = browser.version();
    const context = browser.contexts()[0];
    context.setDefaultTimeout(8000);
    context.setDefaultNavigationTimeout(20_000);
    if (credentials) {
      await context.addCookies(credentials.cookies);
      await context.addInitScript(origins => {
        const state = origins.find(item => item.origin === location.origin);
        for (const item of state?.localStorage || []) localStorage.setItem(item.name, item.value);
      }, credentials.origins);
    }
    const page = await context.newPage();
    await page.setViewportSize(config.device === 'mobile' ? { width: 390, height: 844 } : { width: 1365, height: 900 });
    let errors = 0, failures = 0;
    page.on('pageerror', () => { errors++; });
    page.on('requestfailed', request => {
      if (request.failure()?.errorText === 'net::ERR_ABORTED') return;
      failures++;
      try { if (failures <= 10) finding('resource.failed','error', `Failed ${request.resourceType()} request`, 'Check the resource URL, server response and configured allowed origins', request.url()); } catch { /* Non-HTTP resources are not reported. */ }
    });
    page.on('response', response => {
      if (response.status()<400 || response.request().resourceType()==='document') return;
      try { if (++failures<=10) finding('resource.http','error',`Resource returned HTTP ${response.status()}`,'Restore the resource or update its reference',response.url()); } catch { /* Only HTTP URLs are reported. */ }
    });
    const before = performance.now();
    const response = await page.goto(config.url, { waitUntil: 'domcontentloaded' });
    result.coverage.pagesChecked++;
    result.metrics.push({ name: 'navigation', value: Math.round(performance.now()-before), unit: 'ms', source: 'browser' });
    if (!response) throw new Error('navigation_no_response');
    await resolveTarget(page.url(),allowed);
    const status = response.status();
    if (status >= 400) finding('navigation.status','error',`HTTP ${status}`,'Inspect server logs and the deployment health');
    const loginPage = status === 401 || status === 403 || await page.locator('input[type="password"]').count() > 0;
    if (config.policy === 'private' && loginPage) throw new Error(credentials ? 'auth_profile_expired' : 'auth_profile_required');
    if (config.readySelector) {
      try { await page.locator(config.readySelector).first().waitFor({ state: 'visible' }); }
      catch { finding('journey.ready','error','The configured ready element was not visible','Check application startup and the readiness selector'); }
    }
    for (const step of config.steps || []) {
      const element = page.locator(step.selector).first();
      try {
        if (step.action === 'click') await element.click();
        else if (step.action === 'waitFor') await element.waitFor({ state: 'visible' });
        else if (step.action === 'assertText') {
          const actual = await element.textContent();
          if (!actual?.includes(step.value)) throw new Error('assertion');
        }
      } catch { finding('journey.step','error',`A ${step.action} step did not meet its expectation`,'Review the configured selector, expected state and application behavior'); break; }
    }
    await delay(750);
    if (errors) finding('javascript.exception','error',`${errors} uncaught JavaScript exception(s)`,'Inspect the matching release and sourcemapped errors in Sentori');
    const raw = await response.body();
    if (raw.length > 2*1024*1024) throw new Error('document_size_limit');
    const html = await page.content();
    if (html.length > 2*1024*1024) throw new Error('document_size_limit');
    let anonymousProtected = false;
    if (config.policy === 'private') {
      const anonymous = await browser.newContext({ serviceWorkers: 'block' });
      try {
        const test = await anonymous.newPage();
        const answer = await test.goto(config.url, { waitUntil: 'domcontentloaded', timeout: 15_000 });
        anonymousProtected = [401,403].includes(answer?.status()) || await test.locator('input[type="password"]').count() > 0;
        result.coverage.pagesChecked++;
      } finally { await anonymous.close(); }
    }
    const seo = analyzeSeo({ url: page.url(), status, headers: await response.allHeaders(), rawHtml: raw.toString(), renderedHtml: html, policy: config.policy, anonymousProtected });
    result.findings.push(...seo.findings.map(item => ({ ...item, url: reportUrl(item.url) })));
    async function readDocument(url) {
      if (!allowed.has(new URL(url).origin)) return null;
      const tab = await context.newPage();
      try {
        const answer = await tab.goto(url, { waitUntil: 'domcontentloaded', timeout: 8000 });
        result.coverage.pagesChecked++;
        const body = await answer?.body();
        return { status: answer?.status(), text: body && body.length <= 1024*1024 ? body.toString() : '' };
      } catch { return null; }
      finally { await tab.close(); }
    }
    if (config.policy === 'public') {
      const robots = await readDocument(new URL('/robots.txt', config.url).href);
      const rules = robots?.status === 200 ? analyzeRobots(config.url,robots.text) : { blocked:false,sitemaps:[] };
      if (rules.blocked) finding('seo.robots','error','robots.txt disallows this URL for Googlebot','Allow crawling of this public page; disallow is not a noindex replacement');
      if (!robots || (robots.status >= 500)) finding('seo.robots.unavailable','warning','robots.txt could not be checked','Make robots.txt available to crawlers');
      const sitemapUrls = rules.sitemaps.length ? rules.sitemaps : [new URL('/sitemap.xml',config.url).href];
      let listed = false, checked = false, index = false;
      for (const url of sitemapUrls.slice(0,2)) {
        const sitemap = await readDocument(url);
        if (sitemap?.status === 200) { checked = true; listed ||= sitemapContains(sitemap.text, config.url); index ||= sitemap.text.includes('<sitemapindex'); }
      }
      if (!listed) finding('seo.sitemap','info', index ? 'Sitemap index found; child sitemaps were not crawled' : checked ? 'URL absent from checked sitemap entries' : 'No readable sitemap observed','Verify sitemap coverage; this bounded audit does not establish indexing');
      const links = [...new Set([...seo.links, ...(seo.canonical && allowed.has(new URL(seo.canonical).origin) ? [seo.canonical] : [])])].slice(0,6);
      for (const link of links) {
        const answer = await readDocument(link);
        if (!answer || answer.status >= 400) finding('seo.broken-link','warning',answer ? `Linked URL returned HTTP ${answer.status}` : 'Linked URL did not load','Fix the link or its destination',link);
      }
    }
    if (config.lighthouse) {
      result.coverage.lighthouse = 'failed';
      const mobile = config.device === 'mobile';
      const audit = await lighthouse(config.url, { port: Number(new URL(launched.wsUrl).port), logLevel: 'error', output: 'json' }, {
        extends: 'lighthouse:default', settings: { onlyCategories: config.policy === 'public' ? ['performance','seo'] : ['performance'],
          formFactor: mobile ? 'mobile' : 'desktop', screenEmulation: mobile ? { mobile:true,width:390,height:844,deviceScaleFactor:1,disabled:false } : { mobile:false,width:1365,height:900,deviceScaleFactor:1,disabled:false },
          disableStorageReset: true, maxWaitForLoad: 15_000, throttlingMethod: 'simulate',
          throttling:{rttMs:mobile?150:40,throughputKbps:mobile?1638.4:10240,cpuSlowdownMultiplier:mobile?4:1,requestLatencyMs:0,downloadThroughputKbps:0,uploadThroughputKbps:0} },
      });
      if (!audit?.lhr || audit.lhr.runtimeError) throw new Error('lighthouse_failed');
      result.coverage.lighthouse = 'measured'; result.versions.lighthouse = audit.lhr.lighthouseVersion;
      for (const [name, key, unit, budget] of [['LCP','largest-contentful-paint','ms','lcpMs'],['TBT','total-blocking-time','ms','tbtMs'],['CLS','cumulative-layout-shift','score','cls'],['FCP','first-contentful-paint','ms',null]]) {
        const value = audit.lhr.audits[key]?.numericValue;
        if (!Number.isFinite(value)) continue;
        result.metrics.push({ name, value, unit, source:'lighthouse' });
        if (budget && config.budgets?.[budget] != null && value > config.budgets[budget]) finding(`performance.${name.toLowerCase()}`,'error',`${name} ${Number(value.toFixed(3))} ${unit} exceeds ${config.budgets[budget]} ${unit}`,'Compare repeated runs with the same profile and inspect loading or main-thread work');
      }
      for (const ref of audit.lhr.categories.seo?.auditRefs || []) {
        const item = audit.lhr.audits[ref.id];
        if (item?.score === 0) finding(`lighthouse.${ref.id}`,'warning',item.title,'Review the corresponding Lighthouse SEO audit and retest');
      }
    }
    if (proxy.stats().blocked) finding('network.blocked','warning',`${proxy.stats().blocked} request(s) blocked by destination policy`,'Add only required public origins to the allowlist; private destinations remain blocked');
    result.status = result.findings.some(f => f.severity === 'error') ? 'failed' : 'passed';
  } catch (error) {
    result.status = 'error';
    const known = ['auth_profile_missing','auth_profile_invalid','auth_profile_origin','auth_profile_expired','auth_profile_required','private_destination','origin_not_allowed','document_size_limit','lighthouse_failed','navigation_no_response'];
    const code = known.includes(error?.message) ? error.message : 'browser_check_unavailable';
    finding(`monitor.${code}`,'warning',code,'Check runner health, target access and the configured test account; this result does not establish a site outage');
  } finally {
    if (watchdog) clearTimeout(watchdog);
    if (browser) await browser.close().catch(() => {});
    await stopChrome(launched);
    if (proxy) { result.coverage.blockedRequests = proxy.stats().blocked; await proxy.close(); }
  }
  result.findings = result.findings.slice(0,100);
  return result;
}

export async function readProfiles(path) {
  if (!path) return [];
  const raw = await readFile(path);
  if (raw.length > 1024*1024) throw new Error('auth profiles exceed 1 MB');
  const profiles = JSON.parse(raw.toString());
  if (!Array.isArray(profiles) || profiles.length > 100) throw new Error('invalid auth profiles');
  return profiles;
}
