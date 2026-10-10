import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { withLiveBrowsers } from './live-browser.mjs';
const results = [];
await withLiveBrowsers('setup', async ({engine, context, page, origin, out, password, call}) => {
  await call('/auth/login',{email:'owner@example.test',password});
  const project = await call('/admin/api/projects',{name:`Connect ${engine}`,platform:'react-native'});
  const key = await call(`/admin/api/projects/${project.id}/tokens`,{name:'layout SDK',scope:'ingest'});
  const route = origin + `/setup?project=${project.id}`;
  await page.goto(route); await page.locator('.flow-frame').waitFor();
  const next = page.locator('.flow-actions > .button').last();
  for (const locale of ['zh','ja','en']) for (const theme of ['light','dark']) for (const width of [320,768,1440,3840]) {
    await page.evaluate(({locale,theme}) => {localStorage.setItem('sentori_locale',locale);localStorage.setItem('sentori-theme',theme);}, {locale,theme});
    await page.setViewportSize({width,height:1100}); await page.reload(); await page.locator('.flow-frame').waitFor();
    let footerY;
    for (const step of ['install','configure','verify']) {
      await page.waitForFunction(step => document.querySelector('.flow-steps button[aria-current]')?.textContent && document.querySelector('.flow-steps button[aria-current]')?.getAttribute('aria-controls') === 'sdk-setup-panel' && document.querySelector('.flow-main-pane h2')?.id,step);
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1),false,`${engine}/${locale}/${theme}/${width}/${step}`);
      const metrics = await page.locator('.flow-frame-footer').evaluate(el => ({y:el.getBoundingClientRect().y-el.closest('.flow-frame').getBoundingClientRect().y,overflow:el.scrollWidth>el.clientWidth+1,buttons:[...el.querySelectorAll('button')].map(button => ({width:button.getBoundingClientRect().width,height:button.getBoundingClientRect().height}))}));
      assert.equal(metrics.overflow,false); assert.ok(metrics.buttons.every(button => button.height >= 44)); assert.ok(Math.abs(metrics.buttons[0].width-metrics.buttons[1].width) < 1);
      if (width >= 960 && footerY !== undefined) assert.ok(Math.abs(footerY-metrics.y)<1,JSON.stringify({step,footerY,metrics}));
      footerY = metrics.y;
      if (step === 'configure') {assert.equal(await next.isDisabled(),true); await page.locator('.flow-main-pane input[type=password]').fill(key.token);}
      if (step !== 'verify') await next.click();
    }
    if (locale==='en' && theme==='light' && [320,1440].includes(width)) await page.screenshot({path:out+`${engine}-${width}.png`});
    results.push({engine,locale,theme,width}); writeFileSync(out+'results.json',JSON.stringify({results},null,2));
  }
  await page.goto(route); await page.locator('.flow-frame').waitFor(); await next.click();
  await page.getByRole('button',{name:'Create an ingest key',exact:true}).click();
  await page.waitForFunction(() => document.querySelector('.flow-main-pane input[type=password]')?.value.startsWith('st_'));
  const ingest = await page.locator('.flow-main-pane input[type=password]').inputValue();
  const checkId = await page.evaluate(id => sessionStorage.getItem(`sentori_setup_check:${id}`),project.id);
  await next.click(); await next.click(); await page.getByRole('status').filter({hasText:'No matching test event yet'}).waitFor();
  const report = async contextId => {
    const response = await context.request.post(origin+'/v1/events',{headers:{Authorization:`Bearer ${ingest}`},data:{kind:'error',occurredAt:new Date().toISOString(),platform:'android',release:'myapp@1.0.0',environment:'test',payload:{context:{sentoriSetup:contextId},error:{type:'TypeError',message:'SDK connection '+contextId,stack:[{file:'index.android.bundle',line:1,column:20,function:'e',inApp:true}]}}}});
    assert.equal(response.status(),202,await response.text()); return response.json();
  };
  await report('unrelated-test'); await next.click(); await page.getByRole('status').filter({hasText:'No matching test event yet'}).waitFor();
  const receipt = await report(checkId); await next.click(); await page.getByRole('status').filter({hasText:'Your test event arrived'}).waitFor();
  const source = page.locator('.flow-status-pane li').filter({hasText:'A readable source location is available'});
  assert.equal(await source.locator('svg.text-ok').count(),0);
  const automation = await call(`/admin/api/projects/${project.id}/tokens`,{name:'map upload',scope:'api'});
  const map = {version:3,file:'index.android.bundle',sources:['src/checkout.ts'],sourcesContent:["export function charge(userId) {\n  const token = mintToken(userId)\n  return post('/pay', { token })\n}\n"],names:[],mappings:'oBAEI'};
  const upload = await context.request.post(origin+'/v1/releases/myapp%401.0.0/artifacts',{headers:{Authorization:`Bearer ${automation.token}`},multipart:{kind:'sourcemap',file:{name:'index.android.bundle.map',mimeType:'application/json',buffer:Buffer.from(JSON.stringify(map))}}});
  assert.equal(upload.status(),201,await upload.text()); assert.equal((await upload.json()).usable,true);
  let readable;
  for(let attempt=0;attempt<50;attempt++){readable=(await call('/admin/api/events/'+receipt.eventId)).payload.error.stack[0];if(readable.contextLine)break;await new Promise(resolve=>setTimeout(resolve,100));}
  assert.equal(readable.file,'src/checkout.ts'); await next.click(); await source.locator('svg.text-ok').waitFor();
  await page.getByLabel('App release',{exact:true}).fill('myapp@2.0.0'); await page.getByRole('status').filter({hasText:'Run the test, then check here'}).waitFor(); assert.equal(await source.locator('svg.text-ok').count(),0);
  console.log('PASS',engine,'three-step setup, real key generation, unrelated traffic exclusion, matched receipt, actual source map upload and resymbolication, configuration proof invalidation');
});
console.log(`PASS ${results.length} setup layouts across three steps`);
