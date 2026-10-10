import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { withLiveBrowsers } from './live-browser.mjs';
const results = [];
await withLiveBrowsers('projects', async ({engine, browser, context, page, origin, out, password, call}) => {
  await call('/auth/login', {email: 'owner@example.test', password});
  const aName = `Editable ${engine}`, bName = `Visible ${engine}`;
  const a = await call('/admin/api/projects', {name: aName, platform: 'web'});
  const b = await call('/admin/api/projects', {name: bName});
  const card = name => page.locator('.project-card').filter({has: page.getByRole('heading', {name, exact: true})});
  for (const locale of (process.env.PROJECT_LOCALES?.split(',') ?? ['zh','ja','en'])) for (const theme of ['light','dark']) {
    await page.goto(origin + '/login');
    await page.evaluate(({locale,theme}) => {localStorage.setItem('sentori_locale',locale);localStorage.setItem('sentori-theme',theme);}, {locale,theme});
    for (const width of (process.env.PROJECT_WIDTHS?.split(',').map(Number) ?? [320,768,1440,3840])) {
      await page.setViewportSize({width,height:1000}); await page.goto(origin + '/projects'); await card(aName).waitFor();
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1), false, `${engine}/${locale}/${theme}/${width}`);
      await card(aName).locator('.surface-head button').first().click();
      const dialog = page.locator('dialog[open]'); await dialog.waitFor();
      const metrics = await dialog.evaluate(el => ({width:el.getBoundingClientRect().width, overflow:el.scrollWidth > el.clientWidth + 1, bodyOverflow:el.querySelector('.dialog-content').scrollWidth > el.querySelector('.dialog-content').clientWidth + 1, focused:el.contains(document.activeElement)}));
      assert.ok(metrics.width <= width && !metrics.overflow && !metrics.bodyOverflow && metrics.focused, JSON.stringify(metrics));
      if (locale === 'zh' && theme === 'light' && [320,1440].includes(width)) await page.screenshot({path:out + `${engine}-${width}-edit.png`});
      await page.keyboard.press('Escape'); await dialog.waitFor({state:'hidden'});
      assert.equal(await card(aName).locator('.surface-head button').first().evaluate(el => el === document.activeElement), true);
      results.push({engine,locale,theme,width,metrics});
      writeFileSync(out+'results.json',JSON.stringify({results},null,2));
    }
  }
  await page.setViewportSize({width:768,height:1000});
  await page.evaluate(() => localStorage.setItem('sentori_locale','en')); await page.goto(origin + '/projects'); await card(aName).waitFor();
  await card(aName).getByRole('button',{name:'Edit project',exact:true}).click();
  const dialog = page.locator('dialog[open]');
  await dialog.locator('input').fill('My unsaved edit');
  await call('/admin/api/projects/' + a.id, {name:`Other editor ${engine}`, expectedVersion:1}, 'PATCH');
  await dialog.getByRole('button',{name:'Save',exact:true}).click(); await dialog.getByRole('button',{name:'Reload project'}).waitFor();
  assert.equal(await dialog.locator('input').inputValue(), 'My unsaved edit');
  await dialog.getByRole('button',{name:'Reload project'}).click(); await page.waitForFunction(expected => document.querySelector('dialog[open] input')?.value === expected, `Other editor ${engine}`);
  const edited = `Saved ${engine}`;
  await dialog.locator('input').fill(edited); await dialog.getByRole('button',{name:'Save',exact:true}).click(); await dialog.waitFor({state:'hidden'}); await card(edited).waitFor();
  assert.equal((await call('/admin/api/projects/' + a.id)).version,3);
  await card(edited).getByRole('button',{name:'Delete',exact:true}).click(); await dialog.waitFor();
  assert.equal(await dialog.getByRole('button',{name:'Delete',exact:true}).isDisabled(),true);
  await dialog.locator('input').fill(edited); await dialog.getByRole('button',{name:'Delete',exact:true}).click(); await dialog.waitFor({state:'hidden'}); await card(edited).waitFor({state:'hidden'});
  await call('/admin/api/projects/' + a.id, undefined, 'GET',404);
  await page.locator('.page-heading button').click(); await dialog.waitFor();
  const createdName = `Created through UI ${engine}`;
  await dialog.locator('input').fill(createdName); await dialog.locator('select').selectOption('android'); await dialog.getByRole('button',{name:'Save',exact:true}).click(); await dialog.waitFor({state:'hidden'}); await card(createdName).waitFor();
  const created = (await call('/admin/api/projects')).projects.find(p => p.name === createdName); assert.ok(created && created.platform === 'android');
  await page.locator('input[type=search]').fill(createdName); await page.waitForFunction(() => document.querySelectorAll('.project-card').length === 1); assert.equal(await page.locator('.project-card').count(),1);
  await card(createdName).locator('.surface-foot button').first().click(); await page.waitForURL('**/setup?project=' + created.id); await page.locator('main h1').waitFor();

  const email = `browser-member-${engine}@example.test`;
  const member = await call('/admin/api/users',{email,password});
  await call(`/admin/api/users/${member.id}/projects/${b.id}`, {}, 'PUT');
  const memberContext = await browser.newContext({locale:'en-US'});
  try {
    await call('/auth/login',{email,password},'POST',200,memberContext.request);
    const mp = await memberContext.newPage(); await mp.goto(origin + '/projects'); await mp.locator('.project-card').waitFor();
    assert.equal(await mp.locator('.project-card').count(),1); assert.equal(await mp.locator('.project-card .surface-head button').count(),0);
    await call('/admin/api/projects/' + created.id, undefined, 'GET',403,memberContext.request);
    await mp.goto(origin + '/settings/account?project=' + b.id); await mp.locator('input[type=password]').first().waitFor();
    const fields = mp.locator('input[type=password]'), submit = mp.locator('form button[type=submit]');
    const replacement = `browser-new-password-${engine}-123`;
    await fields.nth(0).fill('wrong-password'); await fields.nth(1).fill(replacement); await fields.nth(2).fill(replacement); await submit.click(); await mp.getByRole('alert').waitFor();
    await fields.nth(0).fill(password); await submit.click(); await mp.getByRole('status').filter({hasText:'Saved'}).waitFor();
    await call('/auth/me', undefined, 'GET',200,memberContext.request);
    const old = await context.request.post(origin + '/auth/login',{data:{email,password}}); assert.equal(old.status(),401);
    await mp.close();
  } finally { await memberContext.close(); }
  writeFileSync(out+'results.json',JSON.stringify({results},null,2));
  console.log('PASS',engine,'project CRUD, stale editor recovery, typed delete, nullable platform, project scopes, member UI and password change');
});
console.log(`PASS ${results.length} project and dialog layouts`);
