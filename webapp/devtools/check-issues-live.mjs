import assert from 'node:assert/strict';
import {writeFileSync} from 'node:fs';
import {withLiveBrowsers} from './live-browser.mjs';
const results=[];
await withLiveBrowsers('issues',async({engine,browser,context,page,origin,out,password,call})=>{
  // Finish the current document's reads before a full fixture navigation; WebKit
  // reports in-flight requests cancelled by page.goto as access-control errors.
  async function visit(url){await page.waitForLoadState('networkidle');await page.goto(url);}
  await call('/auth/login',{email:'owner@example.test',password});
  const project=await call('/admin/api/projects',{name:`Issues ${engine}`,platform:'web'});
  const second=await call('/admin/api/projects',{name:`Other issues ${engine}`,platform:'android'});
  const key=await call(`/admin/api/projects/${project.id}/tokens`,{name:'event fixture',scope:'ingest'});
  const otherKey=await call(`/admin/api/projects/${second.id}/tokens`,{name:'other fixture',scope:'ingest'});
  const report=async(index,token=key.token)=>{
    const response=await context.request.post(origin+'/v1/events',{headers:{Authorization:`Bearer ${token}`},data:{kind:'error',occurredAt:new Date(Date.now()-900000).toISOString(),platform:'android',release:'test@1.0.0',environment:'qa',userId:'test-user',payload:{device:{model:'arm64',os:'ios',osVersion:'27.0'},context:{tenant:'A'},signals:[{t:-6,kind:'tap',data:{x:20,y:30}}],error:{type:'TypeError',message:index===0?'Checkout failed — '+engine:`Problem ${index} — ${engine}`,stack:[{file:index===0?'index.android.bundle':`problem-${index}.js`,line:1,column:20,function:'checkout',inApp:true}]}}}});
    assert.equal(response.status(),202,await response.text());return response.json();
  };
  const deployed=await context.request.post(origin+'/v1/deploys',{headers:{Authorization:`Bearer ${key.token}`},data:{release:'test@1.0.0'}});assert.equal(deployed.status(),202,await deployed.text());
  const receipts=[];for(let index=0;index<55;index++)receipts.push(await report(index));
  const foreign=await report('foreign',otherKey.token);
  const list=(await call(`/admin/api/issues?project_id=${project.id}&status=all`)).issues;
  assert.equal(list.length,55);
  const target=receipts[0],issueId=(await call('/admin/api/events/'+target.eventId)).issueId;
  const member=await call('/admin/api/users',{email:`issues-member-${engine}@example.test`,password});
  await call(`/admin/api/users/${member.id}/projects/${project.id}`,{},'PUT');
  // Real attachment upload. Two rotations and a malformed tail exercise decoding.
  await visit(origin+`/issues?project=${project.id}`);await page.locator('.workbench-document[data-active=true] [data-issue-id]').first().waitFor();
  const png=await page.evaluate(()=>{const canvas=document.createElement('canvas');canvas.width=1000;canvas.height=500;const ctx=canvas.getContext('2d');ctx.fillStyle='#6a8bef';ctx.fillRect(0,0,1000,500);ctx.fillStyle='#fff';ctx.fillRect(100,100,200,200);return canvas.toDataURL('image/png').split(',')[1];});
  const attachment=await context.request.post(origin+`/v1/events/${target.eventId}/attachments/screens`,{headers:{Authorization:`Bearer ${key.token}`},multipart:{source:'js',file:{name:'screens.ndjson',mimeType:'application/x-ndjson',buffer:Buffer.from([{t:-9,mediaType:'image/png',base64:png,w:500,h:1000},{t:-6,mediaType:'image/png',base64:png,w:1000,h:500}].map(JSON.stringify).join('\n')+'\n{"t":')}}});
  assert.equal(attachment.status(),202,await attachment.text());
  const queue=origin+`/issues?project=${project.id}`;
  const detail=origin+`/issues/${issueId}?project=${project.id}`;
  const release=(await call(`/admin/api/projects/${project.id}/releases`)).releases[0];assert.ok(release);
  const releaseUrl=origin+`/releases?project=${project.id}&detail=${release.id}`;
  async function checkDetail(locale,theme,width) {
    await page.getByText('Arm64 · iOS 27.0',{exact:true}).waitFor();
    const geometry=await page.locator('.issue-detail').evaluate(root=>{
      const rect=e=>e.getBoundingClientRect(),center=r=>r.y+r.height/2;
      const type=e=>{const s=getComputedStyle(e);return [s.fontFamily,s.fontSize,s.fontWeight,s.fontStyle,s.lineHeight].join('|');};
      const textOffset=e=>{const r=document.createRange();r.selectNodeContents(e.querySelector('.button-label,.badge-label')??e);return center(r.getBoundingClientRect())-center(rect(e));};
      const badges=[...root.querySelectorAll('.page-heading > div:last-child > span')];
      const actions=[...root.querySelectorAll('.issue-actions button')];
      const toolbar=root.querySelector('.issue-event-toolbar'),select=toolbar.querySelector('select'),raw=toolbar.querySelector('button');
      const code=root.querySelector('[data-evidence-code]'),grid=root.querySelector('.issue-evidence-grid'),timeline=root.querySelector('[data-issue-timeline]');
      const left=rect(root).left;
      return {badges:badges.map(e=>({offset:textOffset(e),height:rect(e).height,font:type(e)})),
        actions:actions.map(e=>({offset:textOffset(e),height:rect(e).height,font:type(e)})),
        insets:[toolbar,root.querySelector('[data-event-id]'),code,grid,timeline].filter(e=>e.getClientRects().length).map(e=>rect(e).left-left),
        selectBeforeRaw:!!(select.compareDocumentPosition(raw)&Node.DOCUMENT_POSITION_FOLLOWING),
        documentWidth:rect(root.closest('.workbench-documents')).width,stackedGap:rect(raw).top-rect(select).bottom,
        controlHeight:rect(select).height,sameControlBaseline:Math.abs(rect(select).bottom-rect(raw).bottom),
        nestedScroll:getComputedStyle(grid.parentElement).overflowY,
        evidenceGap:rect(grid).top-rect(root.querySelector('[data-event-id]')).bottom};
    });
    assert.equal(geometry.badges.length,2);
    for(const badge of geometry.badges)assert.ok(Math.abs(badge.offset)<.75&&badge.height===24,JSON.stringify({engine,locale,width,geometry}));
    assert.equal(new Set(geometry.badges.map(x=>x.font)).size,1);
    assert.equal(geometry.actions.length,2);
    assert.equal(new Set(geometry.actions.map(x=>x.font)).size,1);
    for(const action of geometry.actions)assert.ok(Math.abs(action.offset)<.75&&action.height===geometry.controlHeight,JSON.stringify(geometry));
    assert.ok(geometry.insets.every(inset=>Math.abs(inset)<1),JSON.stringify(geometry));
    assert.ok(geometry.selectBeforeRaw&&geometry.evidenceGap>=12&&geometry.evidenceGap<=24,JSON.stringify(geometry));
    assert.ok(!['auto','scroll'].includes(geometry.nestedScroll));
    if(width>640&&geometry.documentWidth>760)assert.ok(geometry.sameControlBaseline<1,JSON.stringify(geometry));
    else assert.ok(geometry.stackedGap>=12,JSON.stringify(geometry));
    results.push({engine,locale,theme,width,name:'detail-controls',geometry});
    if(locale==='zh'&&[320,1440].includes(width))await page.screenshot({path:out+`${engine}-detail-${locale}-${theme}-${width}.png`});
  }
  // Empty results use the same production CSS as populated queues. Check the
  // actual geometry so a style left only in the design preview cannot pass.
  for(const locale of ['zh','ja','en']) for(const theme of ['light','dark']) for(const width of [320,1440]) {
    await page.evaluate(({locale,theme})=>{localStorage.setItem('sentori_locale',locale);localStorage.setItem('sentori-theme',theme);},{locale,theme});
    await page.setViewportSize({width,height:1100});
    await visit(queue+'&q=no-matching-issue-fixture');
    await page.locator('.queue-empty').waitFor();
    await page.locator('.crash-free-empty').waitFor();
    const metrics=await page.locator('.table-pagination').evaluate(el=>{
      const box=el.getBoundingClientRect(),count=el.firstElementChild.getBoundingClientRect(),actions=el.lastElementChild.getBoundingClientRect();
      const table=el.closest('.surface').getBoundingClientRect(),stats=document.querySelector('.crash-free-card').getBoundingClientRect();
      const empty=document.querySelector('.queue-empty'),button=empty.querySelector('button').getBoundingClientRect(),hint=empty.querySelectorAll('p')[1].getBoundingClientRect();
      return {left:count.left-box.left,right:box.right-actions.right,bottom:box.bottom-actions.bottom,sectionGap:stats.top-table.bottom,actionGap:button.top-hint.bottom,footerOverflow:el.scrollWidth>el.clientWidth+1};
    });
    assert.ok(metrics.left>=12&&metrics.right>=12&&metrics.bottom>=8&&metrics.sectionGap>=20&&metrics.actionGap>=8&&!metrics.footerOverflow,`${engine}/${locale}/${theme}/${width}: ${JSON.stringify(metrics)}`);
    const shortcuts=page.locator('.table-shortcuts');await shortcuts.locator('summary').click();
    assert.equal(await shortcuts.locator('dl > div').count(),5);assert.equal(await shortcuts.locator('kbd').count(),6);
    const help=await shortcuts.locator('.table-shortcuts-panel').evaluate(el=>({left:el.getBoundingClientRect().left,right:el.getBoundingClientRect().right,overflow:el.scrollWidth>el.clientWidth+1}));
    assert.ok(help.left>=0&&help.right<=width&&!help.overflow,JSON.stringify(help));
    assert.equal(await page.locator('.queue-empty p,.crash-free-empty p').evaluateAll(elements=>elements.some(el=>el.textContent.trim().endsWith('。'))),false);
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1),false);
    if(locale==='zh')await page.screenshot({path:out+`${engine}-empty-${theme}-${width}.png`});
    await page.keyboard.press('Escape');assert.equal(await shortcuts.getAttribute('open'),null);
    await visit(detail);await page.locator('[data-event-id]').waitFor();await checkDetail(locale,theme,width);
    results.push({engine,locale,theme,width,name:'empty-queue',metrics});writeFileSync(out+'results.json',JSON.stringify({results},null,2));
  }
  const locales=process.env.VIS_TEST_WORKFLOW_ONLY?[]:process.env.VIS_TEST_QUICK?['en']:['zh','ja','en'];
  const widths=process.env.VIS_TEST_QUICK?[320,1440]:[320,768,1440,3840];
  for(const locale of locales)for(const theme of ['light','dark'])for(const width of widths){
    await page.evaluate(({locale,theme})=>{localStorage.setItem('sentori_locale',locale);localStorage.setItem('sentori-theme',theme);},{locale,theme});
    await page.setViewportSize({width,height:1100});
    for(const [name,url,selector]of [['queue',queue,'[data-issue-id]'],['detail',detail,'[data-event-id]'],['occurrences',detail+'&tab=occurrences','[role=tabpanel] [role=button]'],['activity',detail+'&tab=activity','textarea'],['releases',releaseUrl,'.release-detail .file-input']].filter(([name])=>!process.env.VIS_TEST_ROUTES||process.env.VIS_TEST_ROUTES.split(',').includes(name))){
      await visit(url);await page.locator(selector).first().waitFor();
      assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1),false,`${engine}/${locale}/${theme}/${width}/${name}`);
      if(name==='detail')await checkDetail(locale,theme,width);
      if(name==='releases')assert.equal(await page.locator('.release-detail-body').evaluate(el=>el.scrollWidth>el.clientWidth+1),false);
      if(locale==='en'&&theme==='light'&&[320,1440].includes(width))await page.screenshot({path:out+`${engine}-${width}-${name}.png`});
      results.push({engine,locale,theme,width,name});writeFileSync(out+'results.json',JSON.stringify({results},null,2));
    }
  }
  await page.evaluate(()=>localStorage.setItem('sentori_locale','en'));await page.setViewportSize({width:1440,height:1100});
  await visit(queue);await page.locator('.workbench-document[data-active=true] [data-issue-id]').first().waitFor();
  assert.equal(await page.locator('.workbench-document[data-active=true] [data-issue-id]').count(),50);
  const active = page.locator('.workbench-document[data-active=true]');
  const timeHeader = active.locator('th').last();
  assert.equal(await timeHeader.getAttribute('aria-sort'), 'descending');
  const expectOrder = async sort => {
    // Equal sort keys can leave the row sequence unchanged while React is
    // still committing the requested sort. Wait for the control as well.
    await page.waitForFunction(value => [...document.querySelectorAll('.workbench-document[data-active=true] select')].some(select=>select.value===value), sort);
    const expected = (await call(`/admin/api/issues?project_id=${project.id}&status=all&limit=50&sort=${sort}`)).issues.map(row=>row.id);
    await page.waitForFunction(ids => JSON.stringify([...document.querySelectorAll('.workbench-document[data-active=true] [data-issue-id]')].map(el=>el.dataset.issueId)) === JSON.stringify(ids), expected);
    assert.equal(await page.getByLabel('Sort',{exact:true}).inputValue(), sort);
  };
  await expectOrder('newest');
  for (const [column, first, second] of [[1,'title_desc','title_asc'],[2,'kind_desc','kind_asc'],[3,'status_desc','status_asc'],[4,'events_desc','events_asc'],[5,'users_desc','users_asc'],[6,'newest','oldest']]) {
    const header = active.locator('th').nth(column);
    await header.getByRole('button').click();
    await expectOrder(first);
    assert.equal(await header.getAttribute('aria-sort'),'descending');
    await header.getByRole('button').focus(); await page.keyboard.press('Enter');
    await expectOrder(second);
    assert.equal(await header.getAttribute('aria-sort'),'ascending');
  }
  await page.getByRole('button',{name:'Next',exact:true}).click();
  await page.waitForFunction(()=>new URL(location.href).searchParams.get('offset')==='50');
  await timeHeader.getByRole('button').click(); await expectOrder('newest');
  assert.equal(new URL(page.url()).searchParams.has('offset'),false);
  await page.screenshot({path:out+`${engine}-sortable-queue.png`});
  console.log('PASS',engine,'sortable column headers, keyboard toggles, aria-sort, default occurrence recency and pagination reset');
  const firstIds=await page.locator('.workbench-document[data-active=true] [data-issue-id]').evaluateAll(elements=>elements.map(el=>el.dataset.issueId));
  const selectPage=page.getByRole('checkbox',{name:'Select this page',exact:true});
  const rowChoice=page.locator('.workbench-document[data-active=true] .data-table tbody input[type=checkbox]').first();
  await rowChoice.check();assert.equal(await selectPage.evaluate(el=>el.indeterminate),true);
  await selectPage.focus();await page.keyboard.press('Space');assert.equal(await selectPage.isChecked(),true);assert.equal(await selectPage.evaluate(el=>el.indeterminate),false);
  await page.keyboard.press('Space');assert.equal(await rowChoice.isChecked(),false);

  await page.getByRole('button',{name:'Next',exact:true}).click();await page.waitForFunction(()=>document.querySelectorAll('.workbench-document[data-active=true] [data-issue-id]').length===5);
  const lastIds=await page.locator('.workbench-document[data-active=true] [data-issue-id]').evaluateAll(elements=>elements.map(el=>el.dataset.issueId));assert.equal(new Set([...firstIds,...lastIds]).size,55);
  await page.getByRole('button',{name:'Previous',exact:true}).click();await page.waitForFunction(()=>document.querySelectorAll('.workbench-document[data-active=true] [data-issue-id]').length===50);
  await page.getByLabel('Search issues',{exact:true}).fill('Checkout failed');await page.waitForFunction(()=>document.querySelectorAll('.workbench-document[data-active=true] [data-issue-id]').length===1);
  assert.ok(new URL(page.url()).searchParams.get('q'));await page.locator('.workbench-document[data-active=true] [data-issue-id]').click();await page.locator('[data-event-id]').waitFor();
  await page.getByRole('link',{name:'Back to issues'}).click();await page.locator('.workbench-document[data-active=true] [data-issue-id]').waitFor();assert.equal(await page.getByLabel('Search issues',{exact:true}).inputValue(),'Checkout failed');
  // A partial mutation preserves the failed selection and says what happened.
  await visit(queue);await page.locator('.workbench-document[data-active=true] [data-issue-id]').first().waitFor();
  const chosen=await page.locator('.workbench-document[data-active=true] [data-issue-id]').evaluateAll(elements=>elements.slice(0,2).map(el=>el.dataset.issueId));
  await page.route(`**/admin/api/issues/${chosen[1]}/resolve`,route=>route.fulfill({status:503,contentType:'application/json',body:'{"error":"injected_failure"}'}));
  await page.locator('tbody input[type=checkbox]').nth(0).check();await page.locator('tbody input[type=checkbox]').nth(1).check();
  await page.getByRole('button',{name:'Resolve',exact:true}).click();await page.getByRole('alert').filter({hasText:'1 of 2'}).waitFor();
  assert.equal((await call('/admin/api/issues/'+chosen[0])).status,'resolved');assert.equal((await call('/admin/api/issues/'+chosen[1])).status,'open');
  await page.unroute(`**/admin/api/issues/${chosen[1]}/resolve`);
  // Project switches hide old rows before the delayed new response arrives.
  await page.route(`**/admin/api/issues?*`,async route=>{const parsed=new URL(route.request().url());if(parsed.searchParams.get('project_id')===second.id)await new Promise(resolve=>setTimeout(resolve,500));await route.continue();});
  await page.evaluate(next=>{history.pushState({},'',next);dispatchEvent(new PopStateEvent('popstate'));},`/issues?project=${second.id}`);
  await page.waitForTimeout(100);assert.equal(await page.locator('.workbench-document[data-active=true] [data-issue-id]').count(),0);await page.locator('.workbench-document[data-active=true] [data-issue-id]').waitFor();assert.equal(await page.locator('.workbench-document[data-active=true] [data-issue-id]').count(),1);
  await page.unroute(`**/admin/api/issues?*`);
  await visit(detail);await page.locator('[data-event-id]').waitFor();
  await page.getByLabel('Assignee',{exact:true}).selectOption(member.id);await page.waitForFunction(()=>![...document.querySelectorAll('select')].find(el=>el.closest('.field')?.textContent.includes('Assignee'))?.disabled);
  await page.waitForTimeout(150);assert.equal((await call('/admin/api/issues/'+issueId)).assigneeUserId,member.id);
  await page.locator('.issue-detail').getByRole('tab',{name:'Activity',exact:true}).click();await page.getByRole('textbox',{name:'Note',exact:true}).fill('Investigating the checkout');await page.getByRole('button',{name:'Save note'}).click();await page.getByText('Investigating the checkout',{exact:true}).waitFor();
  await page.getByRole('tab',{name:'Evidence',exact:true}).click();await page.getByRole('group',{name:'Session replay',exact:true}).waitFor();
  const replay=page.getByRole('group',{name:'Session replay',exact:true});await replay.getByRole('button',{name:'Previous frame'}).click();await page.waitForTimeout(100);
  let dims=await replay.locator('.replay-frame').evaluate(el=>({width:el.getBoundingClientRect().width,height:el.getBoundingClientRect().height}));assert.ok(Math.abs(dims.width/dims.height-.5)<.01);
  await replay.getByRole('button',{name:'Next frame'}).click();await page.waitForTimeout(100);dims=await replay.locator('.replay-frame').evaluate(el=>({width:el.getBoundingClientRect().width,height:el.getBoundingClientRect().height}));assert.ok(Math.abs(dims.width/dims.height-2)<.01);
  await replay.focus();await page.keyboard.press('Space');await replay.getByRole('button',{name:'Pause',exact:true}).waitFor();await page.keyboard.press('Space');
  await page.locator('.workbench-document[data-active=true] .issue-event-toolbar').getByRole('button',{name:'Raw event',exact:true}).click();await page.locator('dialog[open] .code-block').waitFor();await page.keyboard.press('Escape');
  await visit(detail+'&occurrence='+foreign.eventId);await page.getByRole('alert').filter({hasText:'does not belong'}).waitFor();assert.equal(await page.locator('[data-event-id]').count(),0);
  // Real form upload, one failed attempt, then the source actually becomes readable.
  await visit(releaseUrl);await page.locator('.release-detail .file-input').waitFor();
  const map={version:3,file:'index.android.bundle',sources:['src/checkout.ts'],sourcesContent:["export function charge(userId) {\n  const token = mintToken(userId)\n  return post('/pay', { token })\n}\n"],names:[],mappings:'oBAEI'};
  const dialog=page.locator('.release-detail');await dialog.locator('input[type=file]').setInputFiles({name:'index.android.bundle.map',mimeType:'application/json',buffer:Buffer.from(JSON.stringify(map))});
  const uploadPath=`**/admin/api/projects/${project.id}/releases/${release.id}/artifacts`;
  await page.route(uploadPath,route=>route.request().method()==='POST'?route.fulfill({status:503,contentType:'application/json',body:'{"error":"injected_failure"}'}):route.continue());
  await dialog.getByRole('button',{name:'Upload debug file',exact:true}).click();await dialog.getByRole('alert').waitFor();assert.equal(await dialog.locator('.file-list strong').textContent(),'index.android.bundle.map');
  await page.unroute(uploadPath);await dialog.getByRole('button',{name:'Upload debug file',exact:true}).click();await dialog.getByRole('status').filter({hasText:'File stored and format checked'}).waitFor();
  assert.ok((await call(`/admin/api/projects/${project.id}/releases/${release.id}/artifacts`)).artifacts.some(artifact=>artifact.usable===true));
  for(let attempt=0;attempt<50;attempt++){const event=await call('/admin/api/events/'+target.eventId);if(event.payload.error.stack[0].contextLine)break;await new Promise(resolve=>setTimeout(resolve,100));}
  await visit(detail);await page.locator('[data-evidence-code] .code-block').waitFor();assert.ok((await page.locator('[data-evidence-code] .code-block').innerText()).includes("return post('/pay'"));
  // Wireframe timestamp is relative to the event, not the final captured frame.
  const plain=await report('wireframe');const plainEvent=await call('/admin/api/events/'+plain.eventId),at=Date.parse(plainEvent.occurredAt);
  const wf=await context.request.post(origin+`/v1/events/${plain.eventId}/attachments/replay`,{headers:{Authorization:`Bearer ${key.token}`},multipart:{source:'android',file:{name:'replay.ndjson',mimeType:'application/x-ndjson',buffer:Buffer.from(JSON.stringify({ts:at-10000,kind:'key',width:1000,height:500,nodes:[{x:0,y:0,w:1000,h:500,color:'#123456'}]})+'\n'+JSON.stringify({ts:at-5000,kind:'delta',added:[],changed:[],removed:[]})+'\n{"kind":')}}});assert.equal(wf.status(),202,await wf.text());
  await visit(origin+`/issues/${plainEvent.issueId}?project=${project.id}`);await page.locator('canvas').waitFor();assert.ok((await page.locator('.replay-controls').innerText()).includes('-5.0s'));
  // Only the destructive confirmation is modal; cancelling preserves the selected detail.
  await visit(releaseUrl);await page.locator('.release-detail').waitFor();
  await page.locator('.release-detail').getByRole('button',{name:'Delete release',exact:true}).click();
  await page.locator('dialog[open]:not(.drawer)').waitFor();await page.keyboard.press('Escape');
  await page.locator('.release-detail').waitFor();assert.ok((await call(`/admin/api/projects/${project.id}/releases`)).releases.some(row=>row.id===release.id));
  await page.locator('.release-detail').getByRole('button',{name:'Delete release',exact:true}).click();
  await page.locator('dialog[open]:not(.drawer)').getByRole('button',{name:'Delete release',exact:true}).click();
  await page.waitForFunction(()=>!document.querySelector('dialog[open]'));assert.ok(!(await call(`/admin/api/projects/${project.id}/releases`)).releases.some(row=>row.id===release.id));
  console.log('PASS',engine,'pagination, search/back URL, partial bulk failure, assignee/note, raw evidence, replay rotation, foreign event guard, real upload retry and retro-symbolication, wireframe event clock, delete confirmation');
});
console.log(`PASS ${results.length} issue and release layouts`);
