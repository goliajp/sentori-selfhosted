import test from 'node:test';
import assert from 'node:assert/strict';
import { publicAddress, resolveTarget, safeUrl } from './proxy.mjs';
import { analyzeSeo, analyzeRobots, sitemapContains } from './seo.mjs';
import { authState } from './check.mjs';

test('public targets reject private, mapped, mixed DNS and undeclared redirect origins', async () => {
  for (const ip of ['127.0.0.1','10.1.2.3','169.254.169.254','192.168.1.3','100.64.1.2','::1','::ffff:127.0.0.1','fc00::1','fe80::1','0.0.0.0']) assert.equal(publicAddress(ip),false,ip);
  assert.equal(publicAddress('8.8.8.8'),true);
  const origins = new Set(['https://site.example']);
  const resolve = rows => async () => rows.map(address => ({address}));
  await assert.rejects(resolveTarget('https://site.example',origins,resolve(['8.8.8.8','127.0.0.1'])),/private_destination/);
  await assert.rejects(resolveTarget('https://other.example',origins,resolve(['8.8.8.8'])),/origin_not_allowed/);
  await assert.rejects(resolveTarget('https://user:password@site.example',origins,resolve(['8.8.8.8'])),/invalid_target/);
  assert.equal((await resolveTarget('https://site.example/path',origins,resolve(['8.8.8.8']))).address,'8.8.8.8');
  assert.equal(safeUrl('https://site.example/path?token=secret#fragment'),'https://site.example/path');
});

test('SEO distinguishes public noindex, private auth, rendered metadata and malformed JSON-LD', () => {
  const html = '<html><head><title>Demo</title><meta name="robots" content="noindex"><link rel="canonical" href="https://other.example/"><script type="application/ld+json">broken</script></head><body><a href="/next">Next</a></body></html>';
  const args = {url:'https://site.example/',status:200,headers:{},rawHtml:'<html></html>',renderedHtml:html,policy:'public'};
  const result = analyzeSeo(args);
  for (const rule of ['seo.noindex','seo.title.rendered-only','seo.description','seo.canonical.external','seo.structured-data.syntax']) assert.ok(result.findings.some(f=>f.rule===rule),rule);
  assert.deepEqual(result.links,['https://site.example/next']);
  assert.deepEqual(analyzeSeo({...args,policy:'private',anonymousProtected:true}).findings,[]);
  assert.ok(analyzeSeo({...args,policy:'private',anonymousProtected:false}).findings.some(f=>f.rule==='private.access'));
  assert.equal(analyzeRobots(args.url,'User-agent: *\nDisallow: /\nSitemap: https://site.example/sitemap.xml').blocked,true);
  assert.equal(sitemapContains('<urlset><url><loc>https://site.example/</loc></url></urlset>',args.url),true);
});

test('auth profile names cannot escape project or app scope, or supply other-origin cookies', () => {
  const job = {projectId:'a',appId:'b',configuration:{url:'https://site.example',authProfile:'admin'}};
  const profile = {projectId:'a',appId:'b',name:'admin',storageState:{cookies:[],origins:[]}};
  assert.deepEqual(authState([profile],job),profile.storageState);
  assert.throws(()=>authState([profile],{...job,projectId:'different'}),/auth_profile_missing/);
  assert.throws(()=>authState([profile],{...job,appId:'different'}),/auth_profile_missing/);
  assert.throws(()=>authState([{...profile,storageState:{cookies:[{domain:'.example'}],origins:[]}}],job),/auth_profile_origin/);
});
