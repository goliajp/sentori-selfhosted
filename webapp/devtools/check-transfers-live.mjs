import assert from 'node:assert/strict';
import {readFileSync, mkdirSync, writeFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {randomUUID, createHash} from 'node:crypto';
import {createServer} from 'vite';
import {launchChrome} from '../../scripts/lib/headless-chrome.mjs';
const fixture=JSON.parse(readFileSync(process.env.SENTORI_TRANSFER_BROWSER_STATE,'utf8'));
const engines=await import(process.env.VIS_PLAYWRIGHT_MODULE ?? 'playwright-core');
const root=fileURLToPath(new URL('../',import.meta.url)),out=fileURLToPath(new URL('../../output/v5-validation/transfers/',import.meta.url));
mkdirSync(out,{recursive:true});
const servers=[],results=[],losses=new Map();
const origins=['http://127.0.0.1:13003','http://127.0.0.1:13005'];
const targets=[fixture.source,fixture.target];
try{
 for(let i=0;i<2;i++){
  assert.equal(new URL(targets[i]).hostname,'127.0.0.1');
  const proxy=Object.fromEntries(['/auth','/admin/api','/v1','/healthz'].map(prefix=>[prefix,{target:targets[i],changeOrigin:true,selfHandleResponse:true,configure(proxy){proxy.on('proxyRes',(incoming,request,response)=>{
   const loss=losses.get(i);
   if(loss && request.method==='POST' && request.url.startsWith(loss.path) && incoming.statusCode===200){
    losses.delete(i);const chunks=[];incoming.on('data',chunk=>chunks.push(chunk));incoming.on('end',()=>{loss.value=JSON.parse(Buffer.concat(chunks));response.destroy();});
   }else{response.writeHead(incoming.statusCode,incoming.headers);incoming.pipe(response);}
  });}}]));
  const server=await createServer({root,configFile:root+'vite.config.ts',envDir:'/tmp/sentori-v5-no-env',server:{host:'127.0.0.1',port:Number(new URL(origins[i]).port),strictPort:true,proxy}});await server.listen();servers.push(server);
 }
 for(const engine of (process.env.SENTORI_BROWSER_ENGINE ? [process.env.SENTORI_BROWSER_ENGINE] : ['chromium','webkit'])){
  let browser,running,source,target,stage='login';
  try{
   if(engine==='chromium'){running=await launchChrome();browser=await engines.chromium.connectOverCDP(running.wsUrl);}else browser=await engines.webkit.launch();
   const sc=await browser.newContext({locale:'en-US'}),tc=await browser.newContext({locale:'en-US'}),errors=[];
   for(const ctx of [sc,tc])await ctx.addInitScript(()=>{if(!localStorage.getItem('sentori_locale'))localStorage.setItem('sentori_locale','en');});
   source=await sc.newPage();target=await tc.newPage();
   for(const page of [source,target])page.on('pageerror',error=>{errors.push(error.message);console.error('PAGE ERROR',stage,error.message);});
   async function call(ctx,origin,path,data,status=200,headers={}){
    const response=await ctx.request.fetch(origin+path,{method:data===undefined?'GET':'POST',data,headers});assert.equal(response.status(),status,`${path}: ${await response.text()}`);return response.json();
   }
   for(const [ctx,origin] of [[sc,origins[0]],[tc,origins[1]]])await call(ctx,origin,'/auth/login',{email:'owner@example.test',password:fixture.password});
   const projectName=`Browser migration ${engine} ${randomUUID().slice(0,8)}`;
   const project=(await call(sc,origins[0],'/admin/api/projects',{name:projectName},201)).id;
   const oldToken=(await call(sc,origins[0],`/admin/api/projects/${project}/tokens`,{name:'Old SDK',scope:'ingest'},201)).token;
   const historical={id:randomUUID(),kind:'error',occurredAt:new Date().toISOString(),platform:'javascript',payload:{error:{type:'Error',message:`Historical ${engine}`}}};
   await call(sc,origins[0],'/v1/events',historical,202,{Authorization:`Bearer ${oldToken}`});
   await source.setViewportSize({width:390,height:900});await target.setViewportSize({width:390,height:900});
   await target.goto(origins[1]+'/transfers');await target.getByRole('button',{name:'Save instance identity',exact:true}).waitFor();
   async function download(page,locator){const ready=page.waitForEvent('download');await locator.click();return readFileSync(await (await ready).path());}
   const identity=await download(target,target.getByRole('button',{name:'Save instance identity',exact:true}));
   await source.goto(origins[0]+'/transfers');await source.getByRole('button',{name:'Start a migration',exact:true}).click();
   let dialog=source.getByRole('dialog');await dialog.getByLabel('Target identity file',{exact:true}).setInputFiles({name:'identity.json',mimeType:'application/json',buffer:identity});
   await dialog.getByRole('checkbox',{name:projectName,exact:true}).check();
   const created=source.waitForResponse(r=>r.url().endsWith('/admin/api/transfers')&&r.request().method()==='POST');
   await dialog.getByRole('button',{name:'Start a migration',exact:true}).click();const id=(await (await created).json()).transferId;await dialog.waitFor({state:'hidden'});
   const sr=source.locator(`[data-transfer-id="${id}"]`),tr=target.locator(`[data-transfer-id="${id}"]`);
   const descriptor=await download(source,sr.getByRole('button',{name:'Save migration descriptor',exact:true}));
   assert.equal(JSON.parse(descriptor).targetInstanceId,JSON.parse(identity).id);
   await target.getByRole('button',{name:'Receive a migration',exact:true}).click();dialog=target.getByRole('dialog');
   await dialog.getByLabel('Migration descriptor file',{exact:true}).setInputFiles({name:'transfer.json',mimeType:'application/json',buffer:descriptor});await dialog.getByRole('button',{name:'Receive a migration',exact:true}).click();await dialog.waitFor({state:'hidden'});
   async function exportPackage(phase,lost=false){
    stage=`export ${phase}`;const title=phase==='copy'?'Export initial copy':'Export final copy';await sr.getByRole('button',{name:title,exact:true}).click();const d=source.getByRole('dialog');
    assert.equal(await d.getByRole('button',{name:title,exact:true}).isEnabled(),false);
    const key=await download(source,d.getByRole('button',{name:'Download recovery key',exact:true}));assert.match(key.toString().trim(),/^[a-f0-9]{64}$/);
    await d.getByRole('checkbox').check();let job;
    const loss={path:`/admin/api/transfers/${id}/commands`};if(lost)losses.set(0,loss);
    const response=source.waitForResponse(r=>r.url().endsWith(`/transfers/${id}/commands`)&&r.status()===200);
    // Do not leave a pending response waiter for the deliberately lost reply.
    if(lost)response.catch(()=>{});
    await d.getByRole('button',{name:title,exact:true}).click();
    if(lost){await d.getByRole('button',{name:'Retry',exact:true}).waitFor();assert.equal(await d.getByRole('checkbox').isDisabled(),true);job=loss.value.jobId;await d.getByRole('button',{name:'Retry',exact:true}).click();}
    const receipt=await response;if(!lost)job=(await receipt.json()).jobId;
    await d.waitFor({state:'hidden'});
    const link=sr.locator(`a[href$="/jobs/${job}/download"]`);await link.waitFor({timeout:60000});
    const archive=await download(source,link);assert.ok((await sr.textContent()).includes(createHash('sha256').update(archive).digest('hex')));
    assert.equal(archive.includes(Buffer.from(oldToken)),false);return {key,archive};
   }
   async function importPackage(phase,data,lost=false){
    stage=`import ${phase}`;const title=phase==='copy'?'Import initial copy':'Import final copy';await tr.getByRole('button',{name:title,exact:true}).click();const d=target.getByRole('dialog');
    await d.getByLabel('Recovery key file',{exact:true}).setInputFiles({name:'recovery.key',mimeType:'text/plain',buffer:data.key});
    await d.getByLabel('Encrypted backup file',{exact:true}).setInputFiles({name:'project.sentori',mimeType:'application/octet-stream',buffer:data.archive});
    let committed;
    const loss={path:`/admin/api/transfers/${id}/upload?`};if(lost)losses.set(1,loss);
    await d.getByRole('button',{name:title,exact:true}).click();
    if(lost){await d.getByRole('button',{name:'Retry',exact:true}).waitFor();assert.equal(await d.getByLabel('Encrypted backup file',{exact:true}).isDisabled(),true);assert.equal(await d.getByLabel('Recovery key file',{exact:true}).isDisabled(),true);committed=loss.value.jobId;await d.getByRole('button',{name:'Retry',exact:true}).click();}
    await d.waitFor({state:'hidden'});
    const next=phase==='copy'?'Import final copy':'Activate target';await tr.getByRole('button',{name:next,exact:true}).waitFor({timeout:60000});
    if(committed){const transfers=(await call(tc,origins[1],'/admin/api/transfers')).items;assert.equal(transfers.find(r=>r.id===id).jobs.filter(j=>j.id===committed).length,1);}
   }
   await importPackage('copy',await exportPackage('copy',true),true);
   const plan='Operator: release owner; keep encrypted archives off-instance; back up and reconcile every new target event before reopening source writes.';
   stage='seal';await sr.getByRole('button',{name:'Seal source writes',exact:true}).click();dialog=source.getByRole('dialog');assert.equal(await dialog.getByRole('button',{name:'Seal source writes',exact:true}).isEnabled(),false);
   await dialog.getByLabel('Rollback and data preservation plan',{exact:true}).fill(plan);await dialog.getByRole('checkbox').check();await dialog.getByRole('button',{name:'Seal source writes',exact:true}).click();await dialog.waitFor({state:'hidden'});
   await call(sc,origins[0],'/v1/events',{...historical,id:randomUUID()},503,{Authorization:`Bearer ${oldToken}`});
   await importPackage('final',await exportPackage('final'));
   stage='activate';await tr.getByRole('button',{name:'Activate target',exact:true}).click();dialog=target.getByRole('dialog');await dialog.getByLabel('Rollback and data preservation plan',{exact:true}).fill(plan);await dialog.getByRole('checkbox').check();await dialog.getByRole('button',{name:'Activate target',exact:true}).click();await dialog.waitFor({state:'hidden'});
   await tr.getByRole('button',{name:'Verify received events',exact:true}).waitFor();assert.equal(await tr.getByRole('button',{name:'Verify received events',exact:true}).isEnabled(),false);assert.equal(await tr.getByRole('button',{name:'Complete migration',exact:true}).count(),0);
   stage='SDK event';let transfer=(await call(tc,origins[1],'/admin/api/transfers')).items.find(row=>row.id===id);const mapped=transfer.projectMapping[project];
   // The setup link must select the newly imported target, even after shell boot.
   await tr.getByRole('link').first().click();await target.waitForURL('**/setup?**');assert.equal(new URL(target.url()).searchParams.get('project'),mapped);await target.getByRole('heading',{name:'Your first error, with the context to fix it.',exact:true}).waitFor();await target.waitForLoadState('networkidle');
   await target.goto(origins[1]+'/transfers');await tr.waitFor();
   const newToken=(await call(tc,origins[1],`/admin/api/projects/${mapped}/tokens`,{name:'New SDK',scope:'ingest'},201)).token;
   const live={...historical,id:randomUUID(),occurredAt:new Date().toISOString()};await call(tc,origins[1],'/v1/events',live,202,{Authorization:`Bearer ${newToken}`});
   await tr.getByText(live.id,{exact:true}).waitFor({timeout:15000});await tr.getByRole('button',{name:'Verify received events',exact:true}).click();dialog=target.getByRole('dialog');await dialog.getByRole('button',{name:'Verify received events',exact:true}).click();await dialog.waitFor({state:'hidden'});
   await tr.getByRole('button',{name:'Complete migration',exact:true}).click();dialog=target.getByRole('dialog');assert.equal(await dialog.getByRole('button',{name:'Complete migration',exact:true}).isEnabled(),false);await dialog.getByRole('checkbox').check();await dialog.getByRole('button',{name:'Complete migration',exact:true}).click();await dialog.waitFor({state:'hidden'});
   transfer=(await call(tc,origins[1],'/admin/api/transfers')).items.find(row=>row.id===id);assert.equal(transfer.state,'completed');assert.equal(transfer.verifiedEvents[project],live.id);
   stage='tail configuration';await sr.getByRole('button',{name:'Forward old clients',exact:true}).click();dialog=source.getByRole('dialog');await dialog.getByLabel('Target HTTPS origin',{exact:true}).fill('https://127.0.0.1');await dialog.getByLabel('New target ingestion token',{exact:true}).fill(newToken);await dialog.getByRole('button',{name:'Forward old clients',exact:true}).click();await dialog.getByRole('alert').waitFor();assert.ok((await dialog.textContent()).includes('400'));assert.ok(!(await source.evaluate(()=>JSON.stringify(localStorage)+JSON.stringify(sessionStorage))).includes(newToken));await dialog.getByRole('button',{name:'Close',exact:true}).last().click();
   stage='responsive matrix';
   for(const page of [source,target])for(const locale of ['en','zh','ja'])for(const theme of ['light','dark'])for(const width of [320,768,1440,3840]){
    await page.evaluate(({locale,theme})=>{localStorage.setItem('sentori_locale',locale);localStorage.setItem('sentori-theme',theme);},{locale,theme});await page.setViewportSize({width,height:1000});await page.reload();await page.locator(`[data-transfer-id="${id}"]`).waitFor();await page.waitForLoadState('networkidle');
    assert.equal(await page.evaluate(()=>document.documentElement.lang),locale);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1),false,`${engine}/${locale}/${theme}/${width}`);
    assert.equal(await page.locator('.host-page [role=alert]').count(),0);
    const side=page===source?'source':'target';if(locale==='zh'&&theme==='light'&&[320,1440].includes(width)){await page.screenshot({path:`${out}/${engine}-${side}-${width}.png`,fullPage:true});await page.locator(`[data-transfer-id="${id}"] .host-steps`).scrollIntoViewIfNeeded();await page.screenshot({path:`${out}/${engine}-${side}-progress-${width}.png`,fullPage:true});}results.push({engine,side,locale,theme,width});
   }
   stage='member boundary';await call(sc,origins[0],'/auth/logout',{});await call(sc,origins[0],'/auth/login',{email:'member@example.test',password:fixture.password});await source.evaluate(()=>localStorage.setItem('sentori_locale','en'));await source.reload();await source.getByText('Only the instance owner can manage project migrations',{exact:true}).waitFor();assert.equal(await source.locator('[data-transfer-id]').count(),0);
   assert.deepEqual(errors,[]);await sc.close();await tc.close();console.log(`PASS ${engine}: real browser copy/import/final/activation/live evidence, immutable command and multipart retries, member boundary`);
  }catch(error){console.error('FAILED STAGE',stage);for(const [page,name] of [[source,'source'],[target,'target']])if(page){await page.screenshot({path:`${out}/${engine}-${name}-failure.png`,fullPage:true});writeFileSync(`${out}/${engine}-${name}-failure.html`,await page.content());}throw error;}
  finally{await browser?.close();running?.chrome.kill();running?.chrome.stderr?.destroy();running?.chrome.stdout?.destroy();running?.chrome.unref();}
 }
 writeFileSync(out+'/results.json',JSON.stringify({layouts:results,workflows:['target identity exchange','owner-authorized descriptor registration','saved encryption key','lost export response replay','lost multipart response replay with immutable File and version','final source fence and target import','actual new target event required','SDK setup project scope','explicit completion acknowledgment','tail SSRF rejection and secret not persisted','member cannot list transfers']},null,2));
}finally{for(const server of servers)await server.close();}
