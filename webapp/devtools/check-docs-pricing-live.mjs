import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { withLiveBrowsers } from './live-browser.mjs';
const versions = JSON.parse(readFileSync(new URL('../shared/documents/doc-versions.json',import.meta.url)));
const results = [];
await withLiveBrowsers('docs-pricing', async ({engine,page,origin,out}) => {
 page.setDefaultTimeout(15000);
 await page.addInitScript(() => {
  if(!new URL(location.href).searchParams.has('shortcut-at-commit'))return;
  const observer=new MutationObserver(() => {
   const search=document.querySelector('input[type=search]');
   if(!search || !document.querySelector('.docs-prose'))return;
   observer.disconnect();
   const key=new KeyboardEvent('keydown',{key:'/',bubbles:true,cancelable:true});
   document.body.dispatchEvent(key);
   window.__docsFirstCommit={focused:document.activeElement===search,prevented:key.defaultPrevented};
  });
  observer.observe(document,{childList:true,subtree:true});
 });
 await page.goto(origin+'/docs?shortcut-at-commit=1');
 await page.waitForFunction(()=>window.__docsFirstCommit!==undefined);
 assert.deepEqual(await page.evaluate(()=>window.__docsFirstCommit),{focused:true,prevented:true},`${engine}: docs shortcut must handle the first committed view`);
 await page.goto(origin+'/docs'); await page.locator('.docs-article-header').waitFor();
 for(const locale of (process.env.SITE_LOCALES?.split(',') ?? ['zh','ja','en'])) for(const theme of ['light','dark']) for(const width of (process.env.SITE_WIDTHS?.split(',').map(Number) ?? [320,768,1440,3840])) {
  await page.evaluate(({locale,theme})=>{localStorage.setItem('sentori_locale',locale);localStorage.setItem('sentori-theme',theme);},{locale,theme});
  await page.setViewportSize({width,height:1000});
  for(const route of ['/docs/getting-started/web','/docs/protocol?version=v4.0.0','/docs/_versions','/pricing']) {
   await page.goto(origin+route);await page.locator('main h1').waitFor();await page.evaluate(()=>document.fonts.ready);
   const key=`${engine}/${locale}/${theme}/${width}${route}`;
   const state=await page.evaluate(()=>({overflow:document.documentElement.scrollWidth>innerWidth+1,missingCopy:document.body.innerText.includes('⟦')}));
   assert.equal(state.overflow,false,key);assert.equal(state.missingCopy,false,key);
   if(route==='/pricing') {
    assert.equal(await page.locator('.solution-card').count(),4);
    assert.ok((await page.locator('.segment').first().boundingBox()).height >= 28,key+' billing control height');
    const cards=await page.locator('.solution-card').evaluateAll(nodes=>nodes.map(node=>({top:node.offsetTop,bottom:node.querySelector('button').getBoundingClientRect().bottom,overflow:node.scrollWidth>node.clientWidth+1})));
    for(const a of cards) { assert.equal(a.overflow,false,key);for(const b of cards)if(a.top===b.top)assert.ok(Math.abs(a.bottom-b.bottom)<1,key); }
    await page.locator('.segment').nth(1).click();assert.ok((await page.locator('[data-plan=starter] .solution-price').innerText()).includes('$15.83'),key);
    await page.locator('[data-plan=starter] button').click();await page.locator('dialog[open]').waitFor();
   } else if(route.includes('getting-started')) {
    assert.equal(await page.locator('.docs-version-select select').inputValue(),'current');
    await page.locator('.docs-article-actions button').nth(1).click();await page.locator('dialog[open]').waitFor();
   }
   if(await page.locator('dialog[open]').count()) {assert.equal(await page.locator('dialog[open]').evaluate(el=>el.scrollWidth>el.clientWidth+1),false,key);await page.keyboard.press('Escape');await page.locator('dialog[open]').waitFor({state:'hidden'});}
   if(locale==='en'&&theme==='light'&&[320,1440].includes(width))await page.screenshot({path:out+`${engine}-${width}-${route.includes('pricing')?'pricing':route.includes('_versions')?'versions':route.includes('protocol')?'protocol':'docs'}.png`,fullPage:true});
   results.push({key,...state});writeFileSync(out+'results.json',JSON.stringify({results},null,2));
  }
  console.log('PASS',engine,locale,theme,width);
 }
 await page.setViewportSize({width:1440,height:1000});await page.evaluate(()=>localStorage.setItem('sentori_locale','en'));
 await page.goto(origin+'/docs/getting-started/web?version=v4.0.0');await page.locator('.docs-prose').waitFor();
 const heading=await page.locator('.docs-prose h2').first().getAttribute('id');
 const headingLink=page.locator('.docs-prose h2').first().locator('a');
 const headingURL=new URL(await headingLink.getAttribute('href'),origin);assert.equal(headingURL.searchParams.get('version'),'v4.0.0');assert.equal(headingURL.hash,'#'+heading);
 await page.goto(headingURL.href);await page.locator('.docs-prose').waitFor();await page.waitForFunction(id=>document.activeElement?.id===id,heading);
 await page.evaluate(()=>{Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:async()=>{throw new Error('denied');}}});});
 await page.locator('.docs-article-actions button').first().click();await page.getByRole('status').filter({hasText:'Copy failed'}).waitFor();
 await page.evaluate(()=>{window.__copied='';Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:async text=>{window.__copied=text;}}});});
 await page.locator('.docs-article-actions button').first().click();assert.equal(await page.evaluate(()=>window.__copied),headingURL.href);
 const [download]=await Promise.all([page.waitForEvent('download'),page.locator('.docs-article-actions button').nth(2).click()]);
 const markdown=readFileSync(await download.path(),'utf8');assert.equal(markdown,versions.find(v=>v.id==='v4.0.0').pages.find(p=>p.slug==='getting-started/web').markdown);
 await page.locator('.docs-article-actions button').nth(1).click();await page.locator('dialog[open]').waitFor();
 await page.locator('dialog[open] .code-copy-button').click();assert.equal(await page.evaluate(()=>window.__copied),markdown);await page.keyboard.press('Escape');
 await page.locator('.docs-version-select select').selectOption('v3.19.0');await page.waitForURL('**version=v3.19.0');
 await page.goto(origin+'/docs/dashboard?version=v3.19.0');await page.locator('.state-panel').waitFor();assert.equal(await page.locator('.docs-prose').count(),0);
 await page.goto(origin+'/docs?version=v99.0.0');await page.locator('.state-panel').waitFor();assert.equal(await page.locator('.docs-prose').count(),0);
 await page.goto(origin+'/docs');await page.locator('.docs-prose').waitFor();await page.keyboard.press('/');assert.equal(await page.locator('input[type=search]').evaluate(e=>e===document.activeElement),true);
 await page.locator('input[type=search]').fill('sourcemap');await page.locator('.docs-search-results a').first().waitFor();await page.keyboard.press('Enter');await page.waitForFunction(()=>location.search.includes('version=current'));assert.equal(await page.locator('input[type=search]').inputValue(),'');
 await page.goto(origin+'/pricing');await page.locator('.solution-card').first().waitFor();assert.match(await page.locator('.solutions-preview').innerText(),/proposed|launch/i);
 await page.locator('[data-usage=events]').fill('600000');await page.locator('[data-usage=replays]').fill('6000');await page.locator('[data-usage=ingestGB]').fill('60');assert.match(await page.locator('output').innerText(),/\$89/);
 await page.locator('.segment').nth(1).click();assert.match(await page.locator('output').innerText(),/\$77.5/);
 await page.locator('[data-usage=events]').fill('10000001');assert.doesNotMatch(await page.locator('output').innerText(),/\$/);
 await page.locator('[data-plan=team] button').click();await page.locator('dialog[open]').waitFor();assert.match(await page.locator('.solution-dialog-note').innerText(),/payment|purchase/i);
 await page.locator('dialog[open] .dialog-footer button').last().click();await page.waitForURL('**/docs/getting-started');await page.locator('.docs-prose').waitFor();
 console.log('PASS',engine,'versioned anchors / source / download / search / missing versions / clipboard denial / estimates / prelaunch disclosure / docs CTA');
});
console.log('PASS',results.length,'docs and pricing configurations');
