import assert from 'node:assert/strict';
import {mkdirSync,writeFileSync} from 'node:fs';
import {fileURLToPath} from 'node:url';
import {createServer} from 'vite';
import {launchChrome} from '../../scripts/lib/headless-chrome.mjs';
const engines=await import(process.env.VIS_PLAYWRIGHT_MODULE ?? 'playwright-core');
const root=fileURLToPath(new URL('../',import.meta.url));
const out=fileURLToPath(new URL('../../output/v5-validation/cloud/',import.meta.url));mkdirSync(out,{recursive:true});
const origin='http://127.0.0.1:13008', api='http://127.0.0.1:13009', mailbox='http://127.0.0.1:13014/messages', control='http://127.0.0.1:13015';
const server=await createServer({root,configFile:root+'vite.config.ts',envDir:'/tmp/sentori-v5-no-env',server:{host:'127.0.0.1',port:13008,strictPort:true,proxy:{'/cloud/api':{target:api,changeOrigin:true}}}});await server.listen();
const results=[];
async function wait(fn) {for(let i=0;i<150;i++){const value=await fn();if(value)return value;await new Promise(r=>setTimeout(r,200));}throw Error('Cloud fixture timeout');}
try {
 for (const engine of ['chromium','webkit']) {
  let browser,running,page;
  try {
   if(engine==='chromium'){running=await launchChrome();browser=await engines.chromium.connectOverCDP(running.wsUrl);}else browser=await engines.webkit.launch();
   const context=await browser.newContext({locale:'en-US'}),errors=[];page=await context.newPage();
   page.on('pageerror',e=>{errors.push(e.message);console.error('PAGE ERROR',e.message);});await context.addInitScript(()=>{if(!localStorage.getItem('sentori_locale'))localStorage.setItem('sentori_locale','en');});
   const stamp=crypto.randomUUID().slice(0,8),email=`owner-${engine}-${stamp}@example.test`,password='Cloud-browser-valid-'+stamp;
   const call=async(path,body,request=context.request,status=200)=>{const response=await request.fetch(origin+'/cloud/api'+path,{method:body===undefined?'GET':'POST',data:body,headers:{Origin:origin}});assert.equal(response.status(),status,await response.text());return response.json();};
   async function register(target,email,name){
    await target.goto(origin+'/cloud/register');await target.getByLabel(/^Email\s*\*?$/).fill(email);await target.getByRole('button',{name:'Send verification email',exact:true}).click();await target.getByRole('status').filter({hasText:'verification email has been queued'}).waitFor();
    const message=await wait(async()=>{const messages=await(await fetch(mailbox)).json();return messages.findLast(row=>row.to.includes(email));});
    const link=message.text.match(/http:\/\/[^\s]+\/cloud\/verify#purpose=register&token=[a-f0-9]{64}/)[0];
    await target.goto(link);await target.getByLabel(/^Display name\s*\*?$/).fill(name);await target.getByLabel(/^Password\s*\*?$/).fill(password);await target.getByLabel(/^Confirm password\s*\*?$/).fill(password);await target.getByRole('button',{name:'Set a new password',exact:true}).click();await target.getByRole('status').filter({hasText:'password'}).waitFor();assert.equal(new URL(target.url()).hash,'');
    await target.goto(origin+'/cloud/login');await target.getByLabel(/^Email\s*\*?$/).fill(email);await target.getByLabel(/^Password\s*\*?$/).fill(password);await target.getByRole('button',{name:'Sign in',exact:true}).click();await target.waitForURL(origin+'/cloud');
   }
   await register(page,email,'Owner');
   let lost=false;
   await page.route('**/cloud/api/organizations',async route=>{if(route.request().method()==='POST'&&!lost){lost=true;await route.fetch();await route.abort('failed');}else await route.continue();});
   const orgName='Browser organization '+stamp;
   await page.getByLabel(/^Organization name\s*\*?$/).fill(orgName);await page.getByRole('button',{name:'Create organization',exact:true}).click();await page.getByRole('alert').waitFor();await page.getByRole('button',{name:'Create organization',exact:true}).click();await page.getByLabel(/^Workspace name\s*\*?$/).waitFor();
   const orgs=await call('/organizations'),org=orgs.items.find(row=>row.name===orgName);assert.ok(org);assert.equal(orgs.items.filter(row=>row.name===orgName).length,1);
   await page.getByLabel(/^Workspace name\s*\*?$/).fill('Browser workspace '+stamp);await page.getByRole('button',{name:'Create workspace draft',exact:true}).click();await page.getByRole('link',{name:'Manage workspace',exact:true}).click();await page.getByLabel(/^Rename workspace\s*\*?$/).fill('Renamed workspace '+stamp);await page.getByRole('button',{name:'Rename workspace',exact:true}).click();await page.getByRole('heading',{name:'Renamed workspace '+stamp,exact:true}).waitFor();await page.getByRole('button',{name:'Request quote',exact:true}).click();await page.getByRole('checkbox').check();await page.getByRole('button',{name:'Accept quote and prepare checkout',exact:true}).click();await page.getByRole('link',{name:'Continue to checkout',exact:true}).waitFor();
   const checkout=await page.getByRole('link',{name:'Continue to checkout',exact:true}).getAttribute('href');assert.ok(checkout.startsWith('https://checkout.stripe.com/'));
   const workspace=new URL(page.url()).pathname.split('/').at(-1),wp='/organizations/'+org.id+'/workspaces/'+workspace;
   assert.equal((await call(wp)).workspace.status,'checkout');
   await fetch(control+'/pay',{method:'POST'});await wait(async()=>(await call(wp)).workspace.status==='paid');await page.getByRole('button',{name:'Reload instance',exact:true}).click();await page.getByRole('button',{name:'Cancel renewal',exact:true}).click();await page.locator('dialog[open]').getByRole('button',{name:'Cancel renewal',exact:true}).click();await wait(async()=>(await call(wp+'/billing')).checkouts[0].cancelAtPeriodEnd);await page.getByRole('button',{name:'Reload instance',exact:true}).click();await page.getByRole('button',{name:'Resume renewal',exact:true}).click();await page.locator('dialog[open]').getByRole('button',{name:'Resume renewal',exact:true}).click();await wait(async()=>(await call(wp+'/billing')).checkouts[0].cancelAtPeriodEnd===false);
   for(const locale of ['zh','ja','en'])for(const theme of ['light','dark'])for(const width of [320,768,1440,3840]){
    await page.evaluate(({locale,theme})=>{localStorage.setItem('sentori_locale',locale);localStorage.setItem('sentori-theme',theme);},{locale,theme});await page.setViewportSize({width,height:1000});await page.reload();await page.locator('.cloud-console .surface').first().waitFor();
    assert.equal(await page.evaluate(()=>document.documentElement.lang),locale);
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1),false,`${engine}/${locale}/${theme}/${width}`);
    if(locale==='zh'&&theme==='light'&&[320,1440].includes(width))await page.screenshot({path:out+`${engine}-${width}.png`,fullPage:true});
    results.push({engine,locale,theme,width});
   }
   await page.evaluate(()=>localStorage.setItem('sentori_locale','en'));await page.goto(origin+'/cloud?organization='+org.id);await page.getByLabel(/^Email\s*\*?$/).fill(`member-${engine}-${stamp}@example.test`);await page.getByRole('button',{name:'Invite member',exact:true}).click();
   const memberContext=await browser.newContext({locale:'en-US'});await memberContext.addInitScript(()=>{if(!localStorage.getItem('sentori_locale'))localStorage.setItem('sentori_locale','en');});const memberPage=await memberContext.newPage();await register(memberPage,`member-${engine}-${stamp}@example.test`,'Member');await memberPage.getByRole('button',{name:'Accept invitation',exact:true}).click();await memberPage.getByRole('button',{name:orgName,exact:true}).click();
   await call(wp,undefined,memberContext.request,403);
   const members=(await call('/organizations/'+org.id)).members,member=members.find(row=>row.email.startsWith('member-'));assert.ok(member);
   await page.goto(origin+'/cloud/workspaces/'+workspace+'?organization='+org.id);await page.getByLabel(/^Organization members\s*\*?$/).selectOption(member.id);await page.getByRole('button',{name:'Save access',exact:true}).click();await wait(async()=>(await call(wp)).members.some(row=>row.id===member.id));await call(wp,undefined,memberContext.request);
   await memberPage.goto(origin+'/cloud/workspaces/'+workspace+'?organization='+org.id);await memberPage.getByText('Workspace access',{exact:true}).waitFor();assert.equal(await memberPage.getByText('Subscription and billing',{exact:true}).count(),0);await call(wp+'/billing',undefined,memberContext.request,403);
   await page.getByRole('button',{name:'Revoke access',exact:true}).click();await wait(async()=>!(await call(wp)).members.some(row=>row.id===member.id));await call(wp,undefined,memberContext.request,403);
   await page.goto(origin+'/cloud?organization='+org.id);
   assert.equal(await page.getByRole('button',{name:'Make member',exact:true}).isDisabled(),true);
   await page.getByRole('button',{name:'Make owner',exact:true}).click();await page.locator('dialog[open]').getByRole('button',{name:'Make owner',exact:true}).click();await wait(async()=>(await call('/organizations/'+org.id)).members.find(row=>row.id===member.id)?.role==='owner');
   await call(wp+'/billing',undefined,memberContext.request);
   const memberRow=page.getByRole('listitem').filter({hasText:`member-${engine}-${stamp}@example.test`});await memberRow.getByRole('button',{name:'Make member',exact:true}).click();await page.locator('dialog[open]').getByRole('button',{name:'Make member',exact:true}).click();await wait(async()=>(await call('/organizations/'+org.id)).members.find(row=>row.id===member.id)?.role==='member');
   await call(wp+'/billing',undefined,memberContext.request,403);
   await memberRow.getByRole('button',{name:'Remove member',exact:true}).click();await page.locator('dialog[open]').getByRole('button',{name:'Remove member',exact:true}).click();await wait(async()=>!(await call('/organizations/'+org.id)).members.some(row=>row.id===member.id));await call('/organizations/'+org.id,undefined,memberContext.request,403);
   await page.getByLabel(/^Rename organization\s*\*?$/).fill('Renamed organization '+stamp);await page.getByRole('button',{name:'Rename organization',exact:true}).click();await wait(async()=>(await call('/organizations/'+org.id)).organization.name==='Renamed organization '+stamp);
   await page.goto(origin+'/cloud/account');await page.getByLabel(/^current password\s*\*?$/).fill('incorrect password');await page.getByLabel(/^New password \(at least 12 characters\)\s*\*?$/).fill(password+'-changed');await page.getByLabel(/^Confirm password\s*\*?$/).fill(password+'-changed');await page.getByRole('button',{name:'Change password',exact:true}).click();await page.getByRole('alert').waitFor();assert.equal(new URL(page.url()).pathname,'/cloud/account');await page.getByLabel(/^current password\s*\*?$/).fill(password);await page.getByRole('button',{name:'Change password',exact:true}).click();await page.waitForURL('**/cloud/login');await call('/auth/me',undefined,context.request,401);
   assert.deepEqual(errors,[]);await memberContext.close();await context.close();console.log('PASS '+engine+' real Cloud registration, one-use email, uncertain-command retry, quote/checkout/renewal, member/owner boundaries, role changes, rename, removal and password revocation');
  }catch(error){if(page){await page.screenshot({path:out+engine+'-failure.png',fullPage:true});writeFileSync(out+engine+'-failure.html',await page.content());}throw error;}finally{await browser?.close();running?.chrome.kill();running?.chrome.stderr?.destroy();running?.chrome.stdout?.destroy();running?.chrome.unref();}
 }
 writeFileSync(out+'results.json',JSON.stringify({results},null,2));
}finally{await server.close();}
