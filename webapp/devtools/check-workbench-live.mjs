import {checkWorkbenchDocuments,selectWorkbenchProject} from './workbench-contract.mjs';
import assert from 'node:assert/strict';
import {writeFileSync} from 'node:fs';
import {withLiveBrowsers} from './live-browser.mjs';
const results=[];
await withLiveBrowsers('workbench',async({engine,page,origin,out,password,call})=>{
  page.on('pageerror',error=>console.error('PAGE ERROR',engine,page.url(),error.message));
  // Let the previous document finish its reads before a full navigation. WebKit
  // reports cancelled document requests as access-control page errors.
  const visit=async url=>{await page.waitForLoadState('networkidle');await page.goto(url);await page.waitForLoadState('networkidle');};
  const reload=async()=>{await page.waitForLoadState('networkidle');await page.reload();await page.waitForLoadState('networkidle');};
  await call('/auth/login',{email:'owner@example.test',password});
  const projects=(await call('/admin/api/projects')).projects;
  const project=projects.find(p=>p.name==='Workbench audit'),other=projects.find(p=>p.name==='Other workbench');assert.ok(project&&other);
  await checkWorkbenchDocuments({page,origin,engine,out,project,other,call});
  const apps=(await call(`/admin/api/projects/${project.id}/apps`)).apps;
  const app=apps.find(a=>a.identifier==='client-a');assert.ok(app);
  await visit(origin+`/issues?project=${project.id}`);await page.locator('[data-issue-id]').first().waitFor();
  await page.getByLabel('Apps',{exact:true}).selectOption(app.id);
  // Two edits in one browser turn exercise the race between committed history
  // and React Router's pending render. Neither condition may replace the other.
  await page.evaluate(({environment,release})=>{
    const set=Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set;
    set.call(environment,'production');environment.dispatchEvent(new Event('input',{bubbles:true}));
    set.call(release,'2.0');release.dispatchEvent(new Event('input',{bubbles:true}));
  },{environment:await page.getByLabel('Environment',{exact:true}).elementHandle(),release:await page.getByLabel('Release',{exact:true}).elementHandle()});
  let filters=new URL(page.url()).searchParams;assert.equal(filters.get('env'),'production',page.url());assert.equal(filters.get('rel'),'2.0',page.url());
  await page.waitForFunction(()=>document.querySelector('input[list$="-releases"]')?.value==='2.0');
  await page.getByLabel('Release',{exact:true}).fill('');
  await page.locator('.advanced-filters summary').click();
  await page.getByLabel('Tag source',{exact:true}).selectOption('tags');await page.getByLabel('Tag key',{exact:true}).fill('region');await page.getByLabel('Tag value',{exact:true}).fill('jp');
  await page.getByRole('button',{name:'Add condition',exact:true}).waitFor();
  await page.evaluate(appType=>{
    Object.getOwnPropertyDescriptor(HTMLInputElement.prototype,'value').set.call(appType,'insight');appType.dispatchEvent(new Event('input',{bubbles:true}));
    document.querySelector('.tag-filter-form').requestSubmit();
  },await page.getByLabel('App type',{exact:true}).elementHandle());
  filters=new URL(page.url()).searchParams;assert.equal(filters.get('env'),'production',page.url());assert.equal(filters.get('app'),app.id,page.url());
  assert.deepEqual(JSON.parse(filters.get('tags')).map(tag=>[tag.source,tag.key,tag.value]).sort(),[['context','appType','insight'],['tags','region','jp']],page.url());
  await page.waitForFunction(()=>document.querySelectorAll('[data-issue-id]').length===1);
  assert.match(page.url(),/tags=/);
  await reload();await page.locator('[data-issue-id]').waitFor();assert.equal(await page.getByLabel('Environment',{exact:true}).inputValue(),'production');
  await page.locator('[data-issue-id]').click();await page.locator('[data-event-id]').waitFor();
  await page.getByRole('link',{name:'Back to issues'}).click();await page.locator('[data-issue-id]').waitFor();
  await page.getByRole('button',{name:'Remove appType filter',exact:true}).click();
  await page.getByLabel('Release',{exact:true}).fill('2.0');await page.locator('.queue-empty').waitFor();
  // Switching projects on a detail page really leaves the old issue and its scope.
  const issue=(await call(`/admin/api/issues?project_id=${project.id}&q=Shared`)).issues[0];
  await visit(origin+`/issues/${issue.id}?project=${project.id}&app=${app.id}&env=production`);await page.locator('[data-event-id]').waitFor();
  await selectWorkbenchProject(page,other.id);await page.waitForURL(url=>url.pathname==='/issues'&&url.searchParams.get('project')===other.id);assert.equal(new URL(page.url()).searchParams.has('app'),false);assert.equal(new URL(page.url()).searchParams.has('env'),false);await page.locator('[data-issue-id]').filter({hasText:'Secret failure'}).waitFor();
  // Real plots, accessible values, CSV download, legend, tabs and persisted filters.
  const instruments=origin+`/instruments?project=${project.id}`;
  await visit(instruments);await page.locator('.telemetry-plot svg').first().waitFor();
  const chart=page.locator('.telemetry-chart').first();
  await chart.getByRole('button',{name:'Data table',exact:true}).click();assert.ok(await chart.locator('tbody tr').count()>0);
  const download=page.waitForEvent('download');await chart.getByRole('button',{name:'Export CSV',exact:true}).click();assert.equal((await download).suggestedFilename(),'sentori-metrics.csv');
  await chart.getByRole('button',{name:'Chart',exact:true}).click();await chart.locator('svg').first().waitFor();
  const legend=chart.locator('.chart-legend button').first();await legend.click();assert.equal(await legend.getAttribute('aria-pressed'),'false');await legend.click();
  for(const tab of ['Launch','Assertions','Probes','Trace points','Overview']){await page.getByRole('tab',{name:tab,exact:true}).click();await page.getByRole('tab',{name:tab,exact:true,selected:true}).waitFor();}
  const windowResponse=page.waitForResponse(r=>r.url().includes('/instruments?hours=168')&&r.status()===200);await page.getByLabel('Observation window',{exact:true}).selectOption('168');await (await windowResponse).json();await page.locator('.telemetry-plot svg').first().waitFor();await reload();assert.equal(await page.getByLabel('Observation window',{exact:true}).inputValue(),'168');
  await page.getByLabel('Release',{exact:true}).fill('does-not-exist');await page.locator('.workbench-empty').first().waitFor();
  // Failed reads display an error; retry recovers the actual plot.
  await page.route('**/admin/api/projects/*/instruments?*',route=>route.fulfill({status:503,contentType:'application/json',body:'{"error":"fixture_failure"}'}));
  await visit(instruments);await page.getByRole('alert').waitFor();assert.equal(await page.locator('.telemetry-plot').count(),0);
  await page.unroute('**/admin/api/projects/*/instruments?*');await page.getByRole('button',{name:'Refresh',exact:true}).click();await page.locator('.telemetry-plot svg').first().waitFor();
  // A direct detail remains reachable outside the visible list page.
  const release=(await call(`/admin/api/projects/${project.id}/releases?q=deep-link`)).releases[0];
  await visit(origin+`/releases?project=${project.id}&offset=50&detail=${release.id}`);await page.getByRole('region',{name:'deep-link@1',exact:true}).waitFor();
  assert.equal(await page.locator('dialog[open]').count(),0);
  await page.locator('.release-item-list [aria-current=true]').waitFor();
  assert.equal(await page.locator('.workbench-tab').count(),1);
  await page.getByRole('button',{name:'Back to list',exact:true}).click();await page.waitForURL(url=>!url.searchParams.has('detail'));
  assert.equal(await page.locator('.workbench-tab').count(),1);
  await visit(instruments);await page.locator('.telemetry-plot svg').first().waitFor();
  // Command search is server-backed, keeps project identity and supports Escape.
  await page.keyboard.press('Control+k');await page.getByRole('dialog').getByRole('combobox').fill('Shared failure');await page.getByRole('option').filter({hasText:'Shared failure'}).waitFor();await page.keyboard.press('Escape');await page.getByRole('dialog').waitFor({state:'hidden'});
  // Notification drafts stay in their project; failed writes are visible and roll back.
  await visit(origin+`/settings/notifications?project=${project.id}`);const webhook=page.getByRole('textbox',{name:'Webhook',exact:true});
  await webhook.waitFor();await page.waitForFunction(()=>!document.querySelector('.workbench-document[data-active=true] input[type=url]')?.disabled);await webhook.fill('https://example.test/unsaved');
  await selectWorkbenchProject(page,other.id);await page.waitForFunction(()=>document.querySelector('.workbench-document[data-active=true] input[type=url]')?.value==='');
  const pref=page.locator('.workbench-document[data-active=true] input[type=checkbox]').first();await pref.waitFor();const was=await pref.isChecked();
  await page.route('**/admin/api/notification-prefs',route=>route.request().method()==='PUT'?route.fulfill({status:503,contentType:'application/json',body:'{"error":"fixture_failure"}'}):route.continue());
  await pref.click();await page.getByRole('alert').filter({hasText:'Could not save notification preferences'}).waitFor();assert.equal(await pref.isChecked(),was);await page.unroute('**/admin/api/notification-prefs');
  // Production geometry, with populated instruments and truly empty task/monitor states.
  for(const locale of ['en','zh','ja'])for(const theme of ['light','dark'])for(const width of [320,1440]){
    await page.evaluate(({locale,theme})=>{localStorage.setItem('sentori_locale',locale);localStorage.setItem('sentori-theme',theme);},{locale,theme});await page.setViewportSize({width,height:1100});
    for(const [name,url,ready] of [['instruments',instruments,'.telemetry-plot svg'],['tasks',origin+`/tasks?project=${project.id}`,'.task-results .workbench-empty'],['monitors',origin+`/status?project=${project.id}`,'.workbench-empty']]){
      await visit(url);await page.locator(ready).first().waitFor();
      const geometry=await page.evaluate(()=>({overflow:document.documentElement.scrollWidth>innerWidth+1,heading:parseFloat(getComputedStyle(document.querySelector('.production-page h1')).fontSize),workspace:document.querySelector('#workspace').scrollWidth>document.querySelector('#workspace').clientWidth+1}));
      assert.equal(geometry.overflow,false,JSON.stringify({engine,name,locale,theme,width,geometry}));assert.equal(geometry.workspace,false);assert.equal(geometry.heading,22);
      if(locale==='zh')await page.screenshot({path:out+`${engine}-${name}-${theme}-${width}.png`});
      results.push({engine,name,locale,theme,width,geometry});writeFileSync(out+'results.json',JSON.stringify({results},null,2));
    }
  }
  console.log('PASS',engine,'real App/environment/tag intersections, navigation, charts, CSV, retries and empty workbench layouts');
});
