import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { withLiveBrowsers } from './live-browser.mjs';
const results = [];
await withLiveBrowsers('settings', async ({engine, browser, context, page, origin, out, password, call}) => {
  page.on('pageerror',error=>console.error('PAGE ERROR',engine,error.message));
  await call('/auth/login', {email:'owner@example.test',password});
  const a = await call('/admin/api/projects',{name:`Keys ${engine}`}), b = await call('/admin/api/projects',{name:`Separate ${engine}`});
  const keyA = await call(`/admin/api/projects/${a.id}/tokens`, {name:`SDK ${engine}`,scope:'ingest'});
  const keyB = await call(`/admin/api/projects/${b.id}/tokens`, {name:`Separate key ${engine}`,scope:'api'});
  await page.goto(origin + `/settings/tokens?project=${a.id}`); await page.locator('main h1').waitFor();
  for (const locale of ['zh','ja','en']) for (const theme of ['light','dark']) for (const width of [320,768,1440]) {
    await page.evaluate(({locale,theme}) => {localStorage.setItem('sentori_locale',locale);localStorage.setItem('sentori-theme',theme);}, {locale,theme});
    await page.setViewportSize({width,height:1000}); await page.reload(); try { await page.locator('.data-table tbody tr').waitFor(); } catch(error) { console.error('SETTINGS LAYOUT',engine,locale,theme,width,page.url(),await page.evaluate(()=>({heading:document.querySelector('h1')?.textContent,active:[...document.querySelectorAll('.workbench-document')].map(el=>({id:el.id,active:el.dataset.active,hidden:el.hidden})),alerts:[...document.querySelectorAll('[role=alert]')].map(el=>el.textContent),tabs:[...document.querySelectorAll('.production-page [role=tab]')].map(el=>({label:el.textContent,selected:el.getAttribute('aria-selected')}))})));await page.screenshot({path:out+'layout-failure.png'});throw error; }
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1),false,`${engine}/${locale}/${theme}/${width}`);
    const tab = page.locator('.production-page').getByRole('tab',{selected:true}); await tab.focus(); await page.keyboard.press('End');
    await page.locator('input[type=password]').first().waitFor(); await page.keyboard.press('Home'); await page.locator('.data-table tbody tr').waitFor();
    await page.locator('.surface-head button').click(); await page.locator('dialog[open] form').waitFor();
    assert.equal(await page.locator('dialog[open]').evaluate(el => el.scrollWidth > el.clientWidth + 1),false);
    await page.keyboard.press('Escape'); await page.locator('dialog[open]').waitFor({state:'hidden'});
    if (locale === 'en' && theme === 'light' && width === 320) await page.screenshot({path:out+engine+'-tokens-320.png'});
    results.push({engine,locale,theme,width}); writeFileSync(out+'results.json',JSON.stringify({results},null,2));
  }
  await page.setViewportSize({width:768,height:1000});
  await page.getByLabel('Project',{exact:true}).selectOption(b.id); await page.getByText(`Separate key ${engine}`,{exact:true}).waitFor();
  assert.equal(await page.getByText(`SDK ${engine}`,{exact:true}).count(),0);
  assert.equal(new URL(page.url()).searchParams.get('project'),b.id);
  await page.locator('.surface-head button').click(); const dialog = page.locator('dialog[open]');
  await dialog.getByLabel('token name').fill('Rotation replacement');
  const tokenPath = `**/admin/api/projects/${b.id}/tokens`;
  await page.route(tokenPath,route => route.request().method() === 'POST' ? route.fulfill({status:503,contentType:'application/json',body:'{"error":"fixture_unavailable"}'}) : route.continue());
  await dialog.getByRole('button',{name:'Generate',exact:true}).click(); await dialog.getByRole('alert').waitFor();
  assert.equal(await dialog.getByLabel('token name').inputValue(),'Rotation replacement');
  await page.unroute(tokenPath); await dialog.getByRole('button',{name:'Generate',exact:true}).click();
  await dialog.locator('code').waitFor(); const secret = await dialog.locator('code').textContent(); assert.ok(secret && secret.length > 20);
  await page.evaluate(() => {Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:async () => {throw new Error('denied');}}});});
  await dialog.locator('.dialog-footer button').last().click(); await dialog.getByRole('alert').waitFor(); assert.equal(await dialog.getByText('Copied',{exact:true}).count(),0);
  // Client-side route change while the secret is visible must clear the old scope.
  await page.evaluate(project => {history.pushState(null,'',`/settings/tokens?project=${project}`);dispatchEvent(new PopStateEvent('popstate'));}, a.id);
  await page.getByText(`SDK ${engine}`,{exact:true}).waitFor(); assert.equal(await page.locator('dialog[open]').count(),0); assert.equal((await page.textContent('body')).includes(secret),false);
  await page.getByLabel('Project',{exact:true}).selectOption(b.id); await page.getByText('Rotation replacement',{exact:true}).waitFor();
  const row = page.locator('tbody tr').filter({hasText:'Rotation replacement'}); await row.getByRole('button',{name:'Revoke',exact:true}).click(); await dialog.getByRole('button',{name:'Revoke',exact:true}).click(); await row.getByText('revoked',{exact:true}).waitFor();
  const keys = (await call(`/admin/api/projects/${b.id}/tokens`)).tokens; assert.ok(keys.find(key => key.name === 'Rotation replacement').revokedAt); assert.equal(keys.find(key => key.id === keyB.id).revokedAt,null);
  await page.getByRole('tab',{name:'Admins',exact:true}).click(); await page.locator('.surface-head button').click();
  const email = `settings-member-${engine}@example.test`;
  await dialog.getByLabel('admin email').fill(email); await dialog.locator('input[type=password]').fill(password); await dialog.getByRole('button',{name:'Create admin',exact:true}).click(); await dialog.waitFor({state:'hidden'});
  const memberRow = page.locator('tbody tr').filter({hasText:email}); await memberRow.waitFor(); await memberRow.getByRole('button',{name:'Manage access',exact:true}).click();
  await dialog.getByLabel(`Keys ${engine}`,{exact:true}).click(); await page.waitForFunction(name => !document.querySelector('dialog[open] fieldset')?.disabled && [...document.querySelectorAll('dialog[open] label')].find(label => label.textContent.trim() === name)?.querySelector('input')?.checked, `Keys ${engine}`);
  const member = (await call('/admin/api/users')).users.find(user => user.email === email); assert.deepEqual(member.projects,[a.id]);
  await dialog.locator('.dialog-footer button').click();
  const mc = await browser.newContext({locale:'en-US'});
  try {
    await call('/auth/login',{email,password},'POST',200,mc.request); const mp = await mc.newPage();
    await mp.goto(origin+`/settings/tokens?project=${b.id}`); await mp.locator('.production-page').getByRole('tabpanel').waitFor(); assert.equal(await mp.locator('.data-table').count(),0);
    await call(`/admin/api/projects/${b.id}/tokens`,undefined,'GET',403,mc.request); await call(`/admin/api/tokens/${keyB.id}`,undefined,'DELETE',403,mc.request);
    await mp.getByLabel('Project',{exact:true}).selectOption(a.id); await mp.getByText(`SDK ${engine}`,{exact:true}).waitFor(); assert.equal(await mp.getByRole('tab',{name:'Admins',exact:true}).count(),0);
    await memberRow.getByRole('button',{name:'Delete',exact:true}).click(); await dialog.getByRole('button',{name:'Delete',exact:true}).click(); await memberRow.waitFor({state:'hidden'});
    await call('/auth/me',undefined,'GET',401,mc.request); await mp.close();
  } finally {await mc.close();}
  assert.ok((await call(`/admin/api/projects/${a.id}/tokens`)).tokens.find(key => key.id === keyA.id));
  console.log('PASS',engine,'settings keyboard, scope isolation, create failure recovery, one-time secret cleanup, clipboard denial, rotation/revocation, member creation, assignment and session revocation');
});
console.log(`PASS ${results.length} settings layouts`);
