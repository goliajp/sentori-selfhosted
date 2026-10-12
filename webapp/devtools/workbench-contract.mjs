import assert from 'node:assert/strict';

export async function selectWorkbenchProject(page, id) {
  const picker=page.locator('.workbench-project-picker').getByRole('combobox');
  await picker.click();
  await picker.fill('');
  await page.locator(`[role=option][data-value="${id}"]`).click();
  await page.waitForURL(url=>url.searchParams.get('project')===id);
  await page.waitForFunction(id=>document.querySelector('.workbench-document[data-active=true]')?.id.startsWith(`document-${id}:`),id);
}

/** Behavioral contract for the production adapter, against the disposable API. */
export async function checkWorkbenchDocuments({page,origin,engine,out,project,other,call}) {
  await page.setViewportSize({width:1440,height:1000});
  await page.goto(origin+`/issues?project=${project.id}`);
  const active=page.locator('.workbench-document[data-active=true]');
  const activeIssue=id=>page.waitForFunction(id=>document.querySelector('.workbench-document[data-active=true]')?.id.includes(`:/issues/${id}:`),id);
  await active.locator('[data-issue-id]').first().waitFor();
  const explorer=page.getByRole('complementary',{name:'Resources',exact:true});
  const inspector=page.getByRole('complementary',{name:'Inspector',exact:true});
  await explorer.getByRole('link',{name:/Shared failure/}).waitFor();
  const split=page.getByRole('separator',{name:'Resize Resources',exact:true});
  const initial=Number(await split.getAttribute('aria-valuenow'));
  await split.focus();await page.keyboard.press('ArrowRight');
  assert.equal(Number(await split.getAttribute('aria-valuenow')),initial+16);
  const box=await split.boundingBox();await page.mouse.move(box.x+2,box.y+50);await page.mouse.down();await page.mouse.move(box.x+26,box.y+50);await page.mouse.up();
  assert.equal(Number(await split.getAttribute('aria-valuenow')),initial+40);
  await page.reload();await active.locator('[data-issue-id]').first().waitFor();assert.equal(Number(await split.getAttribute('aria-valuenow')),initial+40);
  await page.getByRole('button',{name:'Reset workspace layout',exact:true}).click();assert.equal(Number(await split.getAttribute('aria-valuenow')),224);
  await page.getByRole('button',{name:'Toggle inspector',exact:true}).click();assert.equal(await inspector.count(),0);
  await page.getByRole('button',{name:'Toggle inspector',exact:true}).click();await inspector.waitFor();

  await active.getByLabel('Search issues',{exact:true}).fill('Shared failure');
  await active.locator('.advanced-filters summary').click();
  await active.getByLabel('Tag key',{exact:true}).fill('unfinished-key');
  await active.locator('.production-page').evaluate(el=>el.scrollTop=180);
  const scroll=await active.locator('.production-page').evaluate(el=>el.scrollTop);
  const queueURL=page.url();
  await page.locator('.studio-nav').getByRole('link',{name:'Releases',exact:true}).click();
  await active.getByRole('heading',{name:'Releases',exact:true}).waitFor();
  assert.equal(await page.locator('.workbench-document[data-active=true]').count(),1);
  let hiddenWrites=0;
  const observe=request=>{if(request.method()==='POST'&&/\/issues\/.+\/(ignore|resolve)$/.test(new URL(request.url()).pathname))hiddenWrites++;};
  page.on('request',observe);await page.locator('.workbench-tabs [aria-selected=true]').focus();await page.keyboard.press('i');await page.waitForTimeout(100);page.off('request',observe);assert.equal(hiddenWrites,0,'hidden queue keyboard shortcuts must be paused');
  await page.locator('.workbench-tabs').getByRole('tab',{name:'Inbox',exact:true}).click();
  assert.equal(page.url(),queueURL);assert.equal(await active.getByLabel('Search issues',{exact:true}).inputValue(),'Shared failure');assert.equal(await active.getByLabel('Tag key',{exact:true}).inputValue(),'unfinished-key');
  assert.equal(await active.locator('.production-page').evaluate(el=>el.scrollTop),scroll);

  // A slow project response must not expose resources from the previous scope.
  await page.route('**/admin/api/issues?*',async route=>{if(new URL(route.request().url()).searchParams.get('project_id')===other.id)await new Promise(resolve=>setTimeout(resolve,300));await route.continue();});
  await selectWorkbenchProject(page,other.id);assert.equal(await explorer.getByRole('link',{name:/Shared failure/}).count(),0);
  await explorer.getByRole('link',{name:/Secret failure/}).waitFor();await page.unroute('**/admin/api/issues?*');
  await page.locator('.workbench-tabs').getByRole('tab',{name:`Inbox · ${project.name}`,exact:true}).click();
  await active.getByLabel('Tag key',{exact:true}).waitFor();assert.equal(await active.getByLabel('Tag key',{exact:true}).inputValue(),'unfinished-key');
  await page.goBack();await page.waitForURL(url=>url.searchParams.get('project')===other.id);
  await page.goForward();await page.waitForURL(url=>url.searchParams.get('project')===project.id);

  const issues=(await call(`/admin/api/issues?project_id=${project.id}&limit=20`)).issues;
  const first=issues.find(row=>row.title==='Shared failure'), second=issues.find(row=>row.id!==first.id);assert.ok(second);
  await explorer.locator(`a[href^="/issues/${first.id}?"]`).click();await activeIssue(first.id);await active.getByRole('tab',{name:'Activity',exact:true}).click();
  await active.getByRole('textbox',{name:'Note',exact:true}).fill('Unsaved investigation A '+engine);
  await explorer.locator(`a[href^="/issues/${second.id}?"]`).click();await activeIssue(second.id);await active.getByRole('tab',{name:'Activity',exact:true}).click();
  await active.getByRole('textbox',{name:'Note',exact:true}).fill('Unsaved investigation B '+engine);
  const duplicateIds=await page.evaluate(()=>{const ids=[...document.querySelectorAll('[id]')].map(el=>el.id);return ids.filter((id,index)=>ids.indexOf(id)!==index);});assert.deepEqual(duplicateIds,[],'cached documents need unique form and accessibility IDs');
  await page.locator('.workbench-tabs').getByRole('tab',{name:'Shared failure',exact:true}).click();await activeIssue(first.id);assert.equal(await active.getByRole('textbox',{name:'Note',exact:true}).inputValue(),'Unsaved investigation A '+engine);
  await active.getByRole('button',{name:'Save note',exact:true}).click();
  await page.locator('.workbench-bottom-tabs').getByRole('tab',{name:'Activity',exact:true}).click();await page.locator('.workbench-bottom-content').getByText('Unsaved investigation A '+engine,{exact:true}).waitFor();
  // Success invalidates the inspector/activity data, without a full page reload.
  assert.equal(await active.getByRole('textbox',{name:'Note',exact:true}).inputValue(),'');
  const tab=page.locator('.workbench-tabs').getByRole('tab',{name:'Shared failure',exact:true});await tab.focus();const before=await page.locator('.workbench-tab').evaluateAll(nodes=>nodes.map(node=>node.dataset.key));await page.keyboard.press('Alt+ArrowLeft');const after=await page.locator('.workbench-tab').evaluateAll(nodes=>nodes.map(node=>node.dataset.key));assert.notDeepEqual(after,before);assert.equal(new Set(after).size,before.length);
  await page.getByRole('button',{name:'Close Shared failure',exact:true}).click();assert.equal(await page.locator('.workbench-tabs').getByRole('tab',{name:'Shared failure',exact:true}).count(),0);await page.locator('.workbench-document[data-active=true]').waitFor();assert.equal(await page.locator('.workbench-document[data-active=true]').count(),1);

  // A completed save in a hidden document updates its own destination. It
  // cannot steal the foreground or merge another project's query parameters.
  await page.goto(origin+`/tasks?project=${other.id}&new=task`);
  const editor=page.getByRole('dialog',{name:'New task',exact:true});await editor.waitFor();await editor.getByRole('textbox',{name:/^Title\s*\*?$/}).fill('Background save '+engine);
  let resume, started;const hold=new Promise(resolve=>resume=resolve), requested=new Promise(resolve=>started=resolve);
  const endpoint=`**/admin/api/projects/${other.id}/tasks/commands`;
  await page.route(endpoint,async route=>{const response=await route.fetch();started();await hold;await route.fulfill({response});});
  await editor.getByRole('button',{name:'Save',exact:true}).click();await requested;
  const foreground=`/releases?project=${project.id}&q=foreground`;
  await page.evaluate(next=>{history.pushState({},'',next);dispatchEvent(new PopStateEvent('popstate'));},foreground);
  await active.getByRole('heading',{name:'Releases',exact:true}).waitFor();assert.equal(await page.locator('dialog[open]').count(),0);
  const completed=page.waitForResponse(response=>response.url().endsWith(`/projects/${other.id}/tasks/commands`)&&response.status()===200);resume();await completed;await page.unroute(endpoint);await page.waitForLoadState('networkidle');assert.equal(new URL(page.url()).pathname+new URL(page.url()).search,foreground);
  const saved=(await call(`/admin/api/projects/${other.id}/tasks?q=${encodeURIComponent('Background save '+engine)}`)).tasks[0];assert.ok(saved);
  const savedTab=page.locator(`.workbench-tab[data-key="${other.id}:/tasks:${saved.id}"] [role=tab]`);await savedTab.click();await page.waitForURL(url=>url.pathname==='/tasks'&&url.searchParams.get('id')===saved.id);assert.equal(new URL(page.url()).searchParams.has('new'),false);assert.equal(new URL(page.url()).searchParams.has('q'),false);await active.getByRole('heading',{name:'Background save '+engine,exact:true}).waitFor();
  assert.equal(await page.locator('dialog[open]').count(),0);

  // Panel overlays close with Escape and return focus to their toggle.
  await page.setViewportSize({width:320,height:900});
  const toggle=page.getByRole('button',{name:'Toggle inspector',exact:true});await toggle.click();await inspector.waitFor();assert.equal(await page.locator('.workbench-center').getAttribute('inert'),'');await page.keyboard.press('Escape');assert.equal(await inspector.count(),0);assert.equal(await toggle.evaluate(el=>el===document.activeElement),true);
  const left=page.getByRole('button',{name:'Toggle resources',exact:true});await left.click();await explorer.waitFor();await page.keyboard.press('Escape');assert.equal(await explorer.count(),0);
  const longName='A project with a very long name — investigation and release workspace '+engine;
  const longProject=await call('/admin/api/projects',{name:longName});await page.reload();
  await selectWorkbenchProject(page,longProject.id);assert.equal(await page.locator('.workbench-project-picker input').inputValue(),longName);
  for(const width of [320,768,1024,1440,1920]){await page.setViewportSize({width,height:1000});await page.waitForTimeout(60);const geometry=await page.evaluate(()=>({page:document.documentElement.scrollWidth<=innerWidth+1,main:document.querySelector('#workspace').scrollWidth<=document.querySelector('#workspace').clientWidth+1,picker:document.querySelector('.workbench-project-picker').getBoundingClientRect().right<=innerWidth,document:document.querySelector('.workbench-document[data-active=true] .production-page').scrollWidth<=document.querySelector('.workbench-document[data-active=true] .production-page').clientWidth+1}));assert.deepEqual(geometry,{page:true,main:true,picker:true,document:true});if(width===320||width===1440)await page.screenshot({path:out+`${engine}-ide-${width}.png`});}
  // Context-menu actions operate on the clicked tab, including background tabs.
  await page.setViewportSize({width:1440,height:1000});
  const tabList=page.locator('.workbench-tabs'), tabButtons=tabList.getByRole('tab');
  const seedTabs=async()=>{
    await page.waitForLoadState('networkidle');await page.goto(origin+`/issues?project=${project.id}`);
    await active.locator('[data-issue-id]').first().waitFor();
    for(const path of ['releases','status','instruments']) {
      await page.locator(`.studio-nav a[href^="/${path}?"]`).click();
      await page.waitForURL(url=>url.pathname===`/${path}`);
      await page.waitForFunction(path=>document.querySelector('.workbench-document[data-active=true]')?.id.includes(`:/${path}:`),path);
      await active.getByRole('heading',{level:1}).waitFor();await page.waitForLoadState('networkidle');
    }
    assert.equal(await tabButtons.count(),4);
  };
  await seedTabs();
  const original=await page.locator('.workbench-tab').evaluateAll(nodes=>nodes.map(node=>node.dataset.key));
  await tabButtons.nth(1).click({button:'right'});
  await page.getByRole('menuitem',{name:'Close tabs to the right',exact:true}).click();
  assert.deepEqual(await page.locator('.workbench-tab').evaluateAll(nodes=>nodes.map(node=>node.dataset.key)),original.slice(0,2));
  await page.waitForFunction(()=>document.querySelectorAll('.workbench-tabs [role=tab]')[1]?.getAttribute('aria-selected')==='true');
  assert.equal(await tabButtons.nth(1).getAttribute('aria-selected'),'true');
  await tabButtons.nth(1).focus();await page.keyboard.press('Shift+F10');
  assert.equal(await page.getByRole('menuitem',{name:'Close tabs to the right',exact:true}).isDisabled(),true);
  await page.screenshot({path:out+`${engine}-tab-menu.png`});
  const menuBounds=await page.getByRole('menu',{name:'Tab actions'}).boundingBox();assert.ok(menuBounds.x>=0&&menuBounds.x+menuBounds.width<=1440);
  await page.keyboard.press('Escape');await page.waitForFunction(()=>document.querySelectorAll('.workbench-tabs [role=tab]')[1]===document.activeElement);assert.equal(await tabButtons.nth(1).evaluate(el=>el===document.activeElement),true);
  await seedTabs();
  await tabButtons.nth(1).click({button:'right'});await page.getByRole('menuitem',{name:'Close tabs to the left',exact:true}).click();
  assert.equal(await tabButtons.count(),3);assert.equal(new URL(page.url()).pathname,'/instruments');
  await tabButtons.first().click({button:'right'});await page.getByRole('menuitem',{name:'Close other tabs',exact:true}).click();
  await page.waitForURL(url=>url.pathname==='/releases');await active.getByRole('heading',{name:'Releases',exact:true}).waitFor();
  assert.equal(await tabButtons.count(),1);assert.equal(new URL(page.url()).pathname,'/releases');
  await tabButtons.first().click({button:'right'});await page.getByRole('menuitem',{name:'Close all tabs',exact:true}).click();
  await page.getByRole('heading',{name:'No open tabs',exact:true}).waitFor();assert.equal(await tabButtons.count(),0);
  await page.getByRole('button',{name:'Reopen last page',exact:true}).click();await active.getByRole('heading',{name:'Releases',exact:true}).waitFor();
  await tabButtons.first().click({button:'middle'});await page.getByRole('heading',{name:'No open tabs',exact:true}).waitFor();
  await page.locator('.studio-nav a[href^="/releases?"]').click();await active.getByRole('heading',{name:'Releases',exact:true}).waitFor();
  assert.equal(await tabButtons.count(),1,'the same route can reopen after closing its final tab');
  assert.equal(await page.locator('.sentori-topbar-identity img:visible').count(),1);
  assert.equal(await page.locator('.sentori-rail-mark').count(),0,'the navigation rail must not repeat the product mark');
  await page.locator('.instance-status-identity').click();
  const instanceDialog=page.getByRole('dialog',{name:'Instance and versions',exact:true});await instanceDialog.waitFor();
  await instanceDialog.getByText('Self-hosted instance',{exact:true}).waitFor();
  await instanceDialog.getByText('Management console version',{exact:true}).waitFor();
  await instanceDialog.getByText('Data service version',{exact:true}).waitFor();
  const system=await call('/admin/api/system');
  if(system.operatorName){await instanceDialog.getByText(system.operatorName,{exact:true}).waitFor();assert.ok((await page.locator('.instance-status-identity').innerText()).includes(system.operatorName));}
  await page.keyboard.press('Escape');
  console.log('PASS',engine,'tab context actions, background selection, keyboard menu, close-all empty state, middle close, single brand and deployment identity');
  await page.setViewportSize({width:1440,height:1100});await page.goto(origin+`/issues?project=${project.id}`);await active.locator('[data-issue-id]').first().waitFor();
  console.log('PASS',engine,'shared IDE panes, resize persistence, retained drafts/scroll, project isolation, paused shortcuts, inspector updates, keyboard tab reorder and mobile overlays');
}
