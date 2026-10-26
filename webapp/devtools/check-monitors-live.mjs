import assert from 'node:assert/strict';
import {writeFileSync} from 'node:fs';
import {withLiveBrowsers} from './live-browser.mjs';
const results=[];
await withLiveBrowsers('monitors',async({engine,page,origin,out,password,call})=>{
  page.on('requestfailed',request=>{if(request.resourceType()==='script')console.error('Failed script',engine,request.url(),request.failure());});
  await call('/auth/login',{email:'owner@example.test',password});
  const project=(await call('/admin/api/projects')).projects.find(p=>p.name==='Monitors browser '+engine);assert.ok(project);
  const path=`/admin/api/projects/${project.id}`,statusUrl=origin+`/status?project=${project.id}`,integrationsUrl=origin+`/integrations?project=${project.id}`;
  for(const locale of (process.env.VIS_TEST_WORKFLOW_ONLY?[]:['zh','ja','en']))for(const theme of ['light','dark'])for(const width of [320,768,1440,3840]){
    await page.goto(statusUrl);await page.locator('[data-monitor-id]').first().waitFor();
    await page.evaluate(({locale,theme})=>{localStorage.setItem('sentori_locale',locale);localStorage.setItem('sentori-theme',theme);},{locale,theme});
    await page.setViewportSize({width,height:1100});
    for(const name of ['status','integrations','monitor-editor','integration-editor']){
      const monitor=name.startsWith('monitor')||name==='status';
      await page.goto(monitor?statusUrl:integrationsUrl);await page.locator(monitor?'[data-monitor-id]':'[data-integration-kind]').first().waitFor();
      if(name==='monitor-editor')await page.locator('.page-heading button').click();
      if(name==='integration-editor')await page.locator('[data-integration-kind=jira] button').first().click();
      if(name.endsWith('editor'))await page.getByRole('dialog').waitFor();
      await page.evaluate(()=>document.fonts.ready);await page.waitForLoadState('networkidle');
      assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1),false,`${engine}/${name}/${locale}/${theme}/${width}`);
      const workspace=page.locator('#workspace');
      if(await workspace.count())assert.equal(await workspace.evaluate(el=>el.scrollWidth>el.clientWidth+1),false,`${name} workspace overflow`);
      if(name.endsWith('editor'))assert.equal(await page.getByRole('dialog').evaluate(el=>el.scrollWidth>el.clientWidth+1),false,`${name} dialog overflow`);
      if(locale==='en'&&theme==='light'&&[320,1440].includes(width))await page.screenshot({path:out+`${engine}-${width}-${name}.png`});
      results.push({engine,name,locale,theme,width});writeFileSync(out+'results.json',JSON.stringify({results},null,2));
    }
  }
  // Set workflow preferences before loading the document. Visiting the app just
  // to set storage, then immediately navigating again, cancels bootstrap reads
  // and can surface WebKit access-control errors from the discarded document.
  await page.addInitScript(()=>{localStorage.setItem('sentori_locale','en');localStorage.setItem('sentori-theme','light');});await page.setViewportSize({width:1440,height:1100});
  await page.goto(statusUrl);await page.getByRole('button',{name:'New monitor',exact:true}).click();const dialog=page.getByRole('dialog');
  await dialog.getByLabel(/^Monitor name/).fill('Browser monitor '+engine);await dialog.getByLabel(/^Health check URL/).fill('https://health.example.com/ready');await dialog.getByLabel('Enabled',{exact:true}).uncheck();
  const endpoint='**'+path+'/monitors/commands',keys=[];let created;
  await page.route(endpoint,async route=>{keys.push(route.request().postDataJSON().commandId);const response=await route.fetch();assert.equal(response.status(),200);created=(await response.json()).id;await route.abort('failed');});
  await dialog.getByRole('button',{name:'Save',exact:true}).click();await dialog.getByRole('alert').waitFor();assert.equal(await dialog.getByLabel(/^Monitor name/).isDisabled(),true);
  await page.unroute(endpoint);await page.route(endpoint,async route=>{keys.push(route.request().postDataJSON().commandId);await route.continue();});
  await dialog.getByRole('button',{name:'Retry',exact:true}).click();await dialog.waitFor({state:'hidden'});assert.equal(keys[0],keys[1]);assert.equal((await call(path+'/monitors')).monitors.length,5);await page.unroute(endpoint);
  const card=page.locator(`[data-monitor-id="${created}"]`);await card.getByRole('button',{name:'Edit monitor',exact:true}).click();await dialog.getByLabel(/^Monitor name/).fill('Keep unsaved draft');
  const m=(await call(path+'/monitors')).monitors.find(m=>m.id===created);
  const configuration={name:'Concurrent change',url:m.url,intervalSeconds:m.intervalSeconds,timeoutSeconds:m.timeoutSeconds,expectedStatus:m.expectedStatus,failureThreshold:m.failureThreshold,enabled:false};
  await call(path+'/monitors/commands',{action:'save',commandId:crypto.randomUUID(),id:created,expectedVersion:1,configuration},'POST',200);
  await dialog.getByRole('button',{name:'Save',exact:true}).click();await dialog.getByRole('alert').waitFor();assert.equal(await dialog.getByLabel(/^Monitor name/).inputValue(),'Keep unsaved draft');await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
  await page.getByRole('button',{name:'Refresh',exact:true}).click();await page.getByRole('heading',{name:'Concurrent change',exact:true}).waitFor();
  await card.getByRole('button',{name:'Edit monitor',exact:true}).click();await dialog.getByRole('button',{name:'Delete',exact:true}).click();await dialog.getByRole('button',{name:'Delete monitor',exact:true}).click();await dialog.waitFor({state:'hidden'});await card.waitFor({state:'detached'});
  // Read errors remove stale rows; persisted incident notifications link back to this project.
  const listRoute='**'+path+'/monitors';await page.route(listRoute,route=>route.fulfill({status:503,contentType:'application/json',body:'{"error":"fixture_failure"}'}));await page.getByRole('button',{name:'Refresh',exact:true}).click();await page.getByRole('alert').waitFor();assert.equal(await page.locator('[data-monitor-id]').count(),0);await page.unroute(listRoute);
  await page.goto(origin+'/inbox?filter=all');await page.locator('[data-inbox-id]').first().waitFor();await page.locator('.team-inbox-title').filter({hasText:'Release readiness'}).first().click();assert.equal(new URL(page.url()).pathname,'/status');await page.locator('[data-monitor-id]').first().waitFor();

  await page.goto(integrationsUrl);const github=page.locator('[data-integration-kind=github]');await github.getByRole('button',{name:'Configure',exact:true}).click();await dialog.getByLabel(/^Repository/).fill('fixture/updated');
  await dialog.getByRole('button',{name:'Save',exact:true}).click();await dialog.waitFor({state:'hidden'});await github.getByText('fixture/updated',{exact:true}).waitFor();
  await github.getByRole('button',{name:'Configure',exact:true}).click();assert.equal(await dialog.getByLabel(/^Credential/).inputValue(),'');await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
  assert.equal(JSON.stringify(await call(path+'/integrations')).includes('browser-github-secret'),false);
  // The disposable database holds deliveries, so this tests durable queueing without sending externally.
  await github.getByRole('button',{name:'Send test',exact:true}).click();await dialog.getByText('This creates a real issue in the configured external project.',{exact:true}).waitFor();
  let deliveryId;const sendRoute='**'+path+'/integrations/*/send',sendKeys=[];
  await page.route(sendRoute,async route=>{sendKeys.push(route.request().postDataJSON().commandId);const response=await route.fetch();assert.equal(response.status(),200);deliveryId=(await response.json()).deliveryId;await route.abort('failed');});
  await dialog.getByRole('button',{name:'Send now',exact:true}).click();await dialog.getByRole('alert').waitFor();await page.unroute(sendRoute);await page.route(sendRoute,async route=>{sendKeys.push(route.request().postDataJSON().commandId);await route.continue();});
  await dialog.getByRole('button',{name:'Retry',exact:true}).click();await dialog.waitFor({state:'hidden'});assert.equal(sendKeys[0],sendKeys[1]);await page.unroute(sendRoute);
  const delivery=(await call(path+'/deliveries')).deliveries.find(d=>d.id===deliveryId);assert.equal(delivery.state,'queued');assert.equal(delivery.attempt,0);
  const uncertain=page.locator('[data-delivery-id]').filter({has:page.getByText('Outcome uncertain',{exact:true})});await uncertain.getByRole('button',{name:'Retry',exact:true}).click();assert.equal(await dialog.getByRole('button',{name:'Retry',exact:true}).isDisabled(),true);await dialog.getByLabel('I checked the destination and accept that retrying may create a duplicate.',{exact:true}).check();assert.equal(await dialog.getByRole('button',{name:'Retry',exact:true}).isDisabled(),false);await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
  console.log('PASS',engine,'monitor save/replay/conflict/delete, failed reads, incident link, credential retention, durable test-send replay and uncertain retry acknowledgement');
});
console.log('PASS',results.length,'monitor/integration layouts');
