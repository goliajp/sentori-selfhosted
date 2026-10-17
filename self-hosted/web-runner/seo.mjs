import { load } from 'cheerio';
import robotsParser from 'robots-parser';
import { safeUrl } from './proxy.mjs';

export function analyzeSeo({ url, status, headers, rawHtml, renderedHtml, policy, anonymousProtected = false }) {
  const findings = [];
  const add = (rule, severity, evidence, advice) => findings.push({ rule, severity, evidence, advice, url: safeUrl(url) });
  const raw = load(rawHtml), rendered = load(renderedHtml);
  const robots = [headers['x-robots-tag'] || '', ...rendered('meta[name="robots"],meta[name="googlebot"]').map((_, el) => rendered(el).attr('content') || '').get()].join(',').toLowerCase();
  const noindex = /\b(noindex|none)\b/.test(robots);
  if (policy === 'private') {
    if (!anonymousProtected) add('private.access', 'error', 'The anonymous check did not encounter an authentication gate', 'Require authentication before serving private application data');
    if (!noindex) add('private.noindex', 'warning', 'No noindex directive observed', 'Send X-Robots-Tag: noindex or a robots noindex meta tag in addition to authentication');
    return { findings, links: [], canonical: null };
  }
  if (status !== 200) add('seo.status', 'error', `HTTP ${status}`, 'Serve the intended indexable page with HTTP 200');
  if (noindex) add('seo.noindex', 'error', 'The page declares noindex', 'Remove noindex only if this page is intended to appear in search results');
  for (const [selector, rule, label] of [['title','title','title'],['meta[name="description"]','description','meta description']]) {
    const elements = rendered(selector);
    const present = selector === 'title' ? elements.text().trim() : (elements.attr('content') || '').trim();
    if (!present) add(`seo.${rule}`, 'warning', `Missing ${label}`, `Provide a descriptive ${label} for this page`);
    if (elements.length > 1) add(`seo.${rule}.duplicate`, 'warning', `Found ${elements.length} ${label} elements`, `Keep one ${label} per page`);
    if (!raw(selector).length && present) add(`seo.${rule}.rendered-only`, 'info', `${label} appears only after JavaScript rendering`, 'Prefer server-rendered metadata when possible');
  }
  let canonical = null;
  const canonicalElements = rendered('link[rel="canonical"]');
  if (canonicalElements.length > 1) add('seo.canonical.multiple','error','Multiple canonical links','Emit a single canonical URL');
  const canonicalValue = canonicalElements.attr('href');
  if (!canonicalValue) add('seo.canonical.missing','warning','No canonical link observed','Specify the preferred canonical URL');
  else {
    try {
      canonical = new URL(canonicalValue, url).href;
      safeUrl(canonical);
      if (new URL(canonical).origin !== new URL(url).origin) add('seo.canonical.external','warning','Canonical points to another origin','Confirm that the external canonical target is intentional');
    } catch { canonical = null; add('seo.canonical.invalid','error','Invalid canonical URL','Use a valid absolute HTTPS canonical URL'); }
  }
  for (const element of rendered('script[type="application/ld+json"]').toArray().slice(0, 20)) {
    try { JSON.parse(rendered(element).text()); }
    catch { add('seo.structured-data.syntax','warning','Invalid JSON-LD syntax','Correct the JSON-LD syntax and validate its schema separately'); break; }
  }
  for (const element of rendered('link[hreflang]').toArray().slice(0, 20)) {
    const lang = rendered(element).attr('hreflang') || '', href = rendered(element).attr('href');
    try {
      if (!/^(x-default|[a-z]{2,3}(-[a-z0-9]{2,8})*)$/i.test(lang) || !href) throw new Error();
      safeUrl(new URL(href, url).href);
    } catch { add('seo.hreflang','warning','Invalid hreflang language or URL','Use a valid language tag and absolute alternate URL'); break; }
  }
  const links = [...new Set(rendered('a[href]').toArray().slice(0, 1000).flatMap(element => {
    try { const link = new URL(rendered(element).attr('href'), url); return link.origin === new URL(url).origin && !link.search && !link.hash ? [safeUrl(link.href)] : []; }
    catch { return []; }
  }))].filter(link => link !== safeUrl(url));
  if (!rendered('a[href]').length) add('seo.links','info','No crawlable anchor links observed','Use anchors with href for links that search engines should follow');
  if (raw('body').text().trim().length < 40 && rendered('body').text().trim().length >= 40) add('seo.rendering','info','Content depends on JavaScript rendering','Check critical content with search engine rendering tools; this audit does not establish indexing');
  return { findings, links: links.slice(0, 5), canonical };
}
export function analyzeRobots(url, text) {
  const rules = robotsParser(new URL('/robots.txt',url).href, text);
  return { blocked: rules.isAllowed(url, 'Googlebot') === false, sitemaps: rules.getSitemaps().slice(0, 2) };
}
export function sitemapContains(xml, url) {
  const parsed = load(xml, { xmlMode: true });
  return parsed('url > loc').toArray().slice(0, 10000).some(el => { try { return safeUrl(parsed(el).text().trim()) === safeUrl(url); } catch { return false; } });
}
