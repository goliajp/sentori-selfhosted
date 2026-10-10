import assert from 'node:assert/strict';
import {readFileSync,mkdirSync,writeFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {createServer} from 'vite';
import {launchChrome} from '../../scripts/lib/headless-chrome.mjs';
const fixture=JSON.parse(readFileSync(process.env.SENTORI_CLOUD_BROWSER_STATE ?? '/tmp/sentori-v5-provision-browser-local.json','utf8'));
const engines=await import(process.env.VIS_PLAYWRIGHT_MODULE ?? 'playwright-core');
const root=fileURLToPath(new URL('../',import.meta.url)),out=fileURLToPath(new URL('../../output/v5-validation/cloud-federation/',import.meta.url));
mkdirSync(out,{recursive:true});
const servers=[],results=[];
try {
 for(const [origin,target,control] of [[fixture.origin,'http://127.0.0.1:13009',true],[fixture.runtimeOrigin,'http://127.0.0.1:13011',false]]){
  const proxy={};for(const prefix of control ? ['/cloud/api']:['/auth','/admin/api','/v1','/healthz'])proxy[prefix]={target,changeOrigin:true};
  const server=await createServer({root,configFile:root+'vite.config.ts',envDir:'/tmp/sentori-v5-no-env',server:{host:'127.0.0.1',port:Number(new URL(origin).port),strictPort:true,proxy}});await server.listen();servers.push(server);
 }
 for(const engine of ['chromium','webkit']){
  let browser,running,page;
  try{
   if(engine==='chromium'){running=await launchChrome();browser=await engines.chromium.connectOverCDP(running.wsUrl);}else browser=await engines.webkit.launch();
   const context=await browser.newContext({locale:'en-US'});await context.addInitScript(()=>{if(!localStorage.getItem('sentori_locale'))localStorage.setItem('sentori_locale','en');});
   await context.addCookies([{name:'sentori_cloud_session',value:fixture.session,domain:'127.0.0.1',path:'/cloud',httpOnly:true,sameSite:'Strict'}]);
   page=await context.newPage();const errors=[];let stage='handoff';page.on('pageerror',error=>{errors.push(error.message);console.error('PAGE ERROR',stage,error.stack);});
   // The identity/session seed belongs only to this disposable fixture. All
   // browser ticket issuance, fragment consumption and runtime login are real.
   await page.goto(fixture.origin+'/cloud/workspaces/'+fixture.workspace+'?organization='+fixture.organization);
   await page.getByRole('button',{name:'Enter workspace',exact:true}).click();await page.waitForURL(fixture.origin+'/cloud/access?**');
   await page.getByRole('button',{name:'Enter workspace',exact:true}).click();await page.waitForURL(fixture.runtimeOrigin+'/');
   assert.equal(new URL(page.url()).hash,'');
   const me=await context.request.get(fixture.runtimeOrigin+'/auth/me');assert.equal(me.status(),200);assert.equal((await me.json()).role,'superadmin');
   const cookies=await context.cookies();assert.ok(cookies.find(row=>row.name==='sentori_session'&&row.httpOnly));assert.ok(!cookies.some(row=>row.name.includes('verifier')));
   assert.ok(!(await page.evaluate(()=>document.cookie)).includes('sentori_session'));
   await page.goto(fixture.runtimeOrigin+'/projects');await page.getByRole('heading',{name:'Durable project',exact:true}).waitFor();
   stage='managed identity';
   for(const path of ['/login','/forgot-password','/reset-password']){
    await page.goto(fixture.runtimeOrigin+path);await page.getByRole('link',{name:'Continue with Cloud',exact:true}).waitFor();assert.equal(await page.locator('input[type=password]').count(),0);await page.waitForLoadState('networkidle');
   }
   await page.goto(fixture.runtimeOrigin+'/settings/account');await page.getByRole('link',{name:'Manage Cloud account',exact:true}).waitFor();assert.equal(await page.locator('input[type=password]').count(),0);await page.waitForLoadState('networkidle');
   await page.goto(fixture.runtimeOrigin+'/settings/users');await page.getByText('Manage membership in Sentori Cloud. Assign projects here after a member first signs in to this workspace.',{exact:true}).waitFor();assert.equal(await page.locator('input[type=email]').count(),0);await page.waitForLoadState('networkidle');
   await page.goto(fixture.runtimeOrigin+'/self-hosted');await page.waitForURL(fixture.runtimeOrigin+'/settings/instance');await page.locator('.surface dl').first().waitFor();await page.waitForLoadState('networkidle');
   stage='responsive matrix';
   for(const locale of ['zh','ja','en'])for(const theme of ['light','dark'])for(const width of [320,768,1440,3840]){
    await page.evaluate(({locale,theme})=>{localStorage.setItem('sentori_locale',locale);localStorage.setItem('sentori-theme',theme);},{locale,theme});await page.setViewportSize({width,height:1000});await page.reload();await page.locator('.surface dl').first().waitFor();await page.waitForLoadState('networkidle');
    assert.equal(await page.evaluate(()=>document.documentElement.lang),locale);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1),false,`${engine}/${locale}/${theme}/${width}`);
    if(locale==='zh'&&theme==='light'&&[320,1440].includes(width))await page.screenshot({path:out+engine+'-'+width+'.png',fullPage:true});results.push({engine,locale,theme,width});
   }
   stage='session revocation';
   if(engine==='webkit'){
    const response=await context.request.post(fixture.origin+'/cloud/api/auth/logout',{data:{},headers:{Origin:fixture.origin}});assert.equal(response.status(),200);
    assert.equal((await context.request.get(fixture.runtimeOrigin+'/auth/me')).status(),401);
    await page.reload();await page.waitForURL(fixture.runtimeOrigin+'/login');await page.getByRole('link',{name:'Continue with Cloud',exact:true}).waitFor();
   } else {
    const response=await context.request.post(fixture.runtimeOrigin+'/auth/logout',{data:{},headers:{Origin:fixture.runtimeOrigin}});assert.equal(response.status(),200);
    assert.equal((await context.request.get(fixture.runtimeOrigin+'/auth/me')).status(),401);
    assert.equal((await context.request.get(fixture.origin+'/cloud/api/auth/me')).status(),200);
   }
   assert.deepEqual(errors,[]);await context.close();console.log('PASS '+engine+' real Cloud browser handoff, HttpOnly one-use verifier, managed identity, quota UI and session revocation');
  }catch(error){if(page){await page.screenshot({path:out+engine+'-failure.png',fullPage:true});writeFileSync(out+engine+'-failure.html',await page.content());}throw error;}finally{await browser?.close();running?.chrome.kill();running?.chrome.stderr?.destroy();running?.chrome.stdout?.destroy();running?.chrome.unref();}
 }
 writeFileSync(out+'results.json',JSON.stringify({results},null,2));
}finally{for(const server of servers)await server.close();}
