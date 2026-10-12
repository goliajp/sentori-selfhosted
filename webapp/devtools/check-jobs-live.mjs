import assert from 'node:assert/strict';
import {writeFileSync} from 'node:fs';
import {withLiveBrowsers} from './live-browser.mjs';
const results=[];
await withLiveBrowsers('jobs',async({engine,page,origin,out,password,call})=>{
  await call('/auth/login',{email:'owner@example.test',password});
  const project=(await call('/admin/api/projects')).projects.find(row=>row.name==='Jobs browser '+engine);assert.ok(project);
  const release=(await call(`/admin/api/projects/${project.id}/releases`)).releases[0];assert.ok(release);
  const path=`/admin/api/projects/${project.id}`,jobsPath=path+`/jobs?releaseId=${release.id}&limit=10`;
  const releaseUrl=origin+`/releases?project=${project.id}&detail=${release.id}`;
  await page.goto(releaseUrl);await page.locator('.release-detail [role=tab]').nth(1).click();await page.locator('[data-job-id]').first().waitFor();
  for(const locale of (process.env.VIS_TEST_WORKFLOW_ONLY?[]:['zh','ja','en']))for(const theme of ['light','dark'])for(const width of [320,768,1440,3840]){
    await page.evaluate(({locale,theme})=>{localStorage.setItem('sentori_locale',locale);localStorage.setItem('sentori-theme',theme);},{locale,theme});
    await page.setViewportSize({width,height:1100});await page.goto(releaseUrl);await page.locator('.release-detail [role=tab]').nth(1).click();await page.locator('[data-job-id]').first().waitFor();
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1),false,`${engine}/${locale}/${theme}/${width}`);
    assert.equal(await page.locator('.release-detail-body').evaluate(el=>el.scrollWidth>el.clientWidth+1),false);
    await page.locator('[data-job-id]').last().scrollIntoViewIfNeeded();
    if(locale==='en'&&theme==='light'&&[320,1440].includes(width))await page.screenshot({path:out+`${engine}-${width}-jobs.png`});
    results.push({engine,locale,theme,width});
    writeFileSync(out+'results.json',JSON.stringify({results},null,2));
  }
  await page.evaluate(()=>localStorage.setItem('sentori_locale','en'));await page.setViewportSize({width:1440,height:1100});await page.goto(releaseUrl);await page.locator('.release-detail [role=tab]').nth(1).click();await page.locator('[data-job-id]').first().waitFor();
  const pending=(await call(jobsPath)).jobs.find(job=>job.state==='queued');assert.ok(pending);
  const card=page.locator(`[data-job-id="${pending.id}"]`);
  await page.route('**'+path+`/jobs/${pending.id}/cancel`,route=>route.fulfill({status:503,contentType:'application/json',body:'{"error":"injected_failure"}'}));
  await card.getByRole('button',{name:'Cancel job',exact:true}).click();await page.getByRole('alert').filter({hasText:'Could not update background jobs'}).waitFor();
  assert.equal((await call(path+'/jobs/'+pending.id)).state,'queued');
  await page.unroute('**'+path+`/jobs/${pending.id}/cancel`);await card.getByRole('button',{name:'Cancel job',exact:true}).click();await card.getByText('Cancelled',{exact:true}).waitFor();
  assert.equal((await call(path+'/jobs/'+pending.id)).state,'cancelled');
  // The server commits, but the browser loses the response. Retrying the button
  // must send the same key and recover its single durable command.
  // Plain HTTP self-hosted instances may not expose randomUUID.
  await page.evaluate(()=>Object.defineProperty(crypto,'randomUUID',{value:undefined,configurable:true}));
  const command='**'+path+`/releases/${release.id}/resymbolicate`;
  const keys=[];let committed;
  await page.route(command,async route=>{keys.push(route.request().postDataJSON().idempotencyKey);const response=await route.fetch();assert.equal(response.status(),202);committed=(await response.json()).jobId;await route.abort('failed');});
  await page.getByRole('button',{name:'Start reprocessing',exact:true}).click();await page.getByRole('alert').filter({hasText:'Could not update background jobs'}).waitFor();assert.ok(committed);
  await page.unroute(command);await page.route(command,async route=>{keys.push(route.request().postDataJSON().idempotencyKey);await route.continue();});
  await page.getByRole('button',{name:'Start reprocessing',exact:true}).click();await page.locator(`[data-job-id="${committed}"]`).getByText('Completed',{exact:true}).waitFor();
  assert.equal(keys.length,2);assert.equal(keys[0],keys[1]);assert.equal((await call(jobsPath)).jobs.length,3);
  await page.unroute(command);
  await page.route('**'+jobsPath,route=>route.fulfill({status:503,contentType:'application/json',body:'{"error":"injected_failure"}'}));
  await page.getByRole('button',{name:'Refresh jobs',exact:true}).click();await page.getByRole('alert').waitFor();assert.equal(await page.locator('[data-job-id]').count(),0);
  await page.unroute('**'+jobsPath);await page.getByRole('button',{name:'Refresh jobs',exact:true}).click();await page.locator('[data-job-id]').first().waitFor();assert.equal(await page.locator('[data-job-id]').count(),3);
  console.log('PASS',engine,'cancel failure/retry, durable response-loss idempotency, completion polling and failed reads');
});
console.log('PASS',results.length,'job layouts');
