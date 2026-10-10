import assert from 'node:assert/strict';
import {writeFileSync} from 'node:fs';
import {withLiveBrowsers} from './live-browser.mjs';
const results=[];
await withLiveBrowsers('tasks',async({engine,page,origin,out,password,call})=>{
  await call('/auth/login',{email:'owner@example.test',password});
  const project=(await call('/admin/api/projects')).projects.find(p=>p.name==='Tasks browser '+engine);assert.ok(project);
  const path=`/admin/api/projects/${project.id}/tasks`,url=origin+`/tasks?project=${project.id}`;
  const initial=await call(path),task=initial.tasks[0];assert.equal(initial.total,55);
  await page.goto(url);await page.locator('[data-task-id]').first().waitFor();
  for(const locale of (process.env.VIS_TEST_WORKFLOW_ONLY?[]:['zh','ja','en']))for(const theme of ['light','dark'])for(const width of [320,768,1440,3840]){
    await page.evaluate(({locale,theme})=>{localStorage.setItem('sentori_locale',locale);localStorage.setItem('sentori-theme',theme);},{locale,theme});
    await page.setViewportSize({width,height:1100});
    for(const [name,href,ready] of [['list',url,'[data-task-id]'],['board',url+'&view=board','[data-task-id]'],['detail',url+'&id='+task.id,'.team-stage-track'],['inbox',origin+'/inbox?filter=all','[data-inbox-id]']]){
      await page.goto(href);await page.locator(ready).first().waitFor();
      assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1),false,`${engine}/${name}/${locale}/${theme}/${width}`);
      const workspace=page.locator('#workspace');
      if(await workspace.count())assert.equal(await workspace.evaluate(el=>el.scrollWidth>el.clientWidth+1),false,`${name} workspace overflow`);
      if(locale==='en'&&theme==='light'&&[320,1440].includes(width))await page.screenshot({path:out+`${engine}-${width}-${name}.png`});
      results.push({engine,name,locale,theme,width});writeFileSync(out+'results.json',JSON.stringify({results},null,2));
    }
  }
  await page.evaluate(()=>localStorage.setItem('sentori_locale','en'));await page.setViewportSize({width:1440,height:1100});await page.goto(url);await page.locator('[data-task-id]').first().waitFor();
  assert.equal(await page.locator('[data-task-id]').count(),50);
  await page.getByRole('button',{name:'Next',exact:true}).click();await page.waitForFunction(()=>document.querySelectorAll('[data-task-id]').length===5);
  await page.getByRole('button',{name:'Previous',exact:true}).click();await page.waitForFunction(()=>document.querySelectorAll('[data-task-id]').length===50);
  await page.evaluate(()=>Object.defineProperty(crypto,'randomUUID',{value:undefined,configurable:true}));
  await page.getByRole('button',{name:'New task',exact:true}).click();const dialog=page.getByRole('dialog');
  await dialog.getByLabel(/^Title/).fill('Real browser task '+engine);
  await dialog.getByLabel('Description',{exact:true}).fill('Preserve this draft after a lost response.');
  const command='**'+path+'/commands',keys=[];let committed;
  await page.route(command,async route=>{keys.push(route.request().postDataJSON().commandId);const response=await route.fetch();assert.equal(response.status(),200);committed=(await response.json()).id;await route.abort('failed');});
  await dialog.getByRole('button',{name:'Save',exact:true}).click();await dialog.getByRole('alert').waitFor();assert.ok(committed);assert.equal(await dialog.getByLabel(/^Title/).inputValue(),'Real browser task '+engine);assert.equal(await dialog.getByLabel(/^Title/).isDisabled(),true);
  await page.unroute(command);await page.route(command,async route=>{keys.push(route.request().postDataJSON().commandId);await route.continue();});
  await dialog.getByRole('button',{name:'Retry',exact:true}).click();await page.locator('.team-stage-track').waitFor();assert.equal(keys.length,2);assert.equal(keys[0],keys[1]);assert.equal((await call(path)).total,56);await page.unroute(command);
  await page.locator('.team-manual-stage summary').click();await page.getByRole('button',{name:'Ready',exact:true}).click();await page.waitForFunction(()=>document.querySelector('.team-stage-track [data-active=true]')?.textContent==='Ready');
  await page.getByLabel(/^Comment/).fill('A real comment');await page.getByRole('button',{name:'Post comment',exact:true}).click();await page.locator('.team-history').getByText('A real comment',{exact:true}).waitFor();
  // Version conflict keeps the entered draft and does not overwrite the latest task.
  await page.getByRole('button',{name:'Edit task',exact:true}).click();await dialog.getByLabel(/^Title/).fill('Unsaved conflict draft');
  const latest=(await call(path+'/'+committed)).task;
  await call(path+'/commands',{action:'comment',commandId:crypto.randomUUID(),taskId:committed,expectedVersion:latest.version,body:'Concurrent author',mention:null},'POST',200);
  await dialog.getByRole('button',{name:'Save',exact:true}).click();await dialog.getByRole('alert').waitFor();assert.equal(await dialog.getByLabel(/^Title/).inputValue(),'Unsaved conflict draft');assert.equal((await call(path+'/'+committed)).task.title,'Real browser task '+engine);
  await dialog.getByRole('button',{name:'Cancel',exact:true}).click();
  // A failed read has an error and retry; it does not keep previous rows visible.
  await page.goto(url);await page.locator('[data-task-id]').first().waitFor();const listRoute='**'+path+'?*';
  await page.route(listRoute,route=>route.fulfill({status:503,contentType:'application/json',body:'{"error":"fixture_failure"}'}));await page.getByRole('button',{name:'Refresh',exact:true}).first().click();await page.getByRole('alert').waitFor();assert.equal(await page.locator('[data-task-id]').count(),0);
  await page.unroute(listRoute);await page.getByRole('button',{name:'Refresh',exact:true}).first().click();await page.locator('[data-task-id]').first().waitFor();
  await page.getByText('Task automation',{exact:true}).first().click();
  await page.getByLabel(/^Repository/).fill('acme/checkout');await page.getByLabel(/^Deployment environment/).fill('production');
  await page.getByRole('button',{name:'Save',exact:true}).click();await page.getByLabel('Signing key',{exact:true}).waitFor();
  const signingKey=await page.getByLabel('Signing key',{exact:true}).inputValue();assert.equal(signingKey.length,64);
  const integration=await call(`/admin/api/projects/${project.id}/task-integration`);assert.equal(integration.configuration.repository,'acme/checkout');assert.equal(JSON.stringify(integration).includes(signingKey),false);
  await page.getByLabel('Replace the signing key; the old key will stop working',{exact:true}).check();await page.getByRole('button',{name:'Save',exact:true}).click();
  await page.waitForFunction(old=>document.querySelector('input[readonly]')?.value!==old,signingKey);
  const rotated=await page.getByLabel('Signing key',{exact:true}).inputValue();assert.equal(rotated.length,64);assert.notEqual(rotated,signingKey);
  await page.reload();await page.locator('[data-task-id]').first().waitFor();await page.getByText('Task automation',{exact:true}).first().click();
  await page.getByLabel('Webhook endpoint',{exact:true}).waitFor();assert.equal(await page.getByLabel('Signing key',{exact:true}).count(),0);
  await page.goto(origin+'/inbox?filter=all');await page.locator('[data-inbox-id]').first().waitFor();const item=page.locator('[data-inbox-id]').first(),itemId=await item.getAttribute('data-inbox-id');
  const update='**/admin/api/inbox/'+itemId;
  await page.route(update,route=>route.fulfill({status:503,contentType:'application/json',body:'{"error":"fixture_failure"}'}));await item.getByRole('button',{name:'Mark read',exact:true}).click();await page.getByRole('alert').waitFor();assert.equal(await item.getAttribute('data-unread'),'true');
  await page.unroute(update);await item.getByRole('button',{name:'Mark read',exact:true}).click();await item.getByRole('button',{name:'Mark unread',exact:true}).waitFor();
  await item.getByRole('button',{name:'Archive',exact:true}).click();await page.locator(`[data-inbox-id="${itemId}"]`).waitFor({state:'detached'});
  await page.getByRole('button',{name:'Notification preferences',exact:true}).click();await dialog.getByLabel('Mentions',{exact:true}).uncheck();await dialog.getByRole('button',{name:'Save',exact:true}).click();await dialog.waitFor({state:'hidden'});assert.equal((await call('/admin/api/inbox-preferences')).mentions,false);
  console.log('PASS',engine,'task pagination, response-loss recovery, workflow, conflicts, failed reads, inbox actions and preferences');
});
console.log('PASS',results.length,'task/inbox layouts');
