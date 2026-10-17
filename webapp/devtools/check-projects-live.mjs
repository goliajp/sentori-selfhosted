import assert from 'node:assert/strict';
import { writeFileSync } from 'node:fs';
import { withLiveBrowsers } from './live-browser.mjs';
const results = [];
await withLiveBrowsers('projects', async ({engine, browser, context, page, origin, out, password, call}) => {
  page.on('pageerror',error=>console.error('PAGE ERROR',engine,page.url(),error.message));
  await call('/auth/login', {email: 'owner@example.test', password});
  const aName = `Editable ${engine}`, bName = `Visible ${engine}`;
  const a = await call('/admin/api/projects', {name: aName, platform: 'web'});
  const b = await call('/admin/api/projects', {name: bName});
  const activeDocument = page.locator('.workbench-document[data-active=true]');
  const card = name => activeDocument.locator('.project-card').filter({has: page.getByRole('heading', {name, exact: true})});
  for (const locale of (process.env.PROJECT_LOCALES?.split(',') ?? ['zh','ja','en'])) for (const theme of ['light','dark']) {
    await page.goto(origin + '/login');
    await page.evaluate(({locale,theme}) => {localStorage.setItem('sentori_locale',locale);localStorage.setItem('sentori-theme',theme);}, {locale,theme});
    for (const width of (process.env.PROJECT_WIDTHS?.split(',').map(Number) ?? [320,768,1440,3840])) {
      await page.setViewportSize({width,height:1000}); await page.goto(origin + '/projects'); await card(aName).waitFor();
      // Health and app loading can move the edit button after the card first appears.
      await activeDocument.locator('.grid[aria-busy=false]').waitFor();
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
  await activeDocument.locator('.grid[aria-busy=false]').waitFor();
  await card(aName).getByRole('button',{name:'Edit project',exact:true}).click();
  const dialog = page.locator('dialog[open]');
  try { await dialog.waitFor(); } catch(error) {
    console.error('PROJECT EDIT',engine,page.url(),await page.evaluate(()=>({
      active:[...document.querySelectorAll('.workbench-document')].map(el=>({id:el.id,active:el.dataset.active,hidden:el.hidden})),
      dialogs:[...document.querySelectorAll('dialog')].map(el=>({open:el.open,title:el.querySelector('h2')?.textContent})),
      focused:document.activeElement?.outerHTML,
    })));
    await page.screenshot({path:out+`${engine}-edit-failure.png`});throw error;
  }
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
  await activeDocument.locator('.page-heading button').click(); await dialog.waitFor();
  const createdName = `Created through UI ${engine}`;
  await dialog.locator('input').fill(createdName); assert.equal(await dialog.locator('select').count(),0); await dialog.getByRole('button',{name:'Save',exact:true}).click(); await dialog.waitFor({state:'hidden'}); await card(createdName).waitFor();
  const created = (await call('/admin/api/projects')).projects.find(p => p.name === createdName); assert.ok(created && created.platform === null);
  assert.deepEqual((await call(`/admin/api/projects/${created.id}/apps`)).apps,[]);
  await card(createdName).getByRole('button',{name:/^iOS/}).click(); await page.waitForURL('**/setup?project='+created.id+'&platform=ios');
  await activeDocument.locator('.flow-frame').waitFor();
  assert.equal(await activeDocument.locator('select').filter({has:page.locator('option[value=ios]')}).inputValue(),'ios');
  await page.goto(origin+'/projects'); await card(createdName).waitFor();
  await activeDocument.locator('input[type=search]').fill(createdName); await page.waitForFunction(() => document.querySelectorAll('.workbench-document[data-active=true] .project-card').length === 1); assert.equal(await activeDocument.locator('.project-card').count(),1);
  await card(createdName).locator('.surface-foot button').first().click(); await page.waitForURL('**/setup?project=' + created.id); await activeDocument.locator('h1').waitFor();

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
  // The SDK continues to address only a project; the server identifies apps.
  const multiName=`Multi-app ${engine}`;
  const multi=await call('/admin/api/projects',{name:multiName});
  const sdk=await call(`/admin/api/projects/${multi.id}/tokens`,{name:'One project SDK key',scope:'ingest'});
  for(const [platform,release] of [['ios','mobile@1.0'],['android','mobile@2.0'],['web','storefront@3.0']]) {
    const response=await context.request.post(origin+'/v1/events',{headers:{Authorization:`Bearer ${sdk.token}`},data:{kind:'error',occurredAt:new Date().toISOString(),platform,release,payload:{message:`${engine} ${platform}`}}});
    assert.equal(response.status(),202,await response.text());
  }
  const apps=(await call(`/admin/api/projects/${multi.id}/apps`)).apps;
  const ios=apps.find(app=>app.platform==='ios');
  assert.equal(apps.length,3);
  const unknownResponse=await context.request.post(origin+'/v1/events',{headers:{Authorization:`Bearer ${sdk.token}`},data:{kind:'error',occurredAt:new Date().toISOString(),platform:'android',release:'unknown',payload:{message:'Missing app identity'}}});
  assert.equal(unknownResponse.status(),202,await unknownResponse.text());
  await page.goto(origin+'/projects'); await card(multiName).locator('[data-app-id]').first().waitFor();
  assert.equal(await card(multiName).locator('[data-app-id]').count(),3);
  assert.equal(await card(multiName).locator('[data-app-count]').textContent(),'3');
  await card(multiName).locator('.unidentified-apps summary').click();
  assert.equal(await card(multiName).locator('[data-unidentified-app-id]').count(),1);
  await card(multiName).getByText('Unidentified app · Android',{exact:true}).waitFor();
  await card(multiName).locator('.unidentified-apps summary').click();
  assert.equal(await card(multiName).getByRole('button',{name:/^iOS/}).getAttribute('title'),'Connected');
  const iosRow=card(multiName).locator(`[data-app-id="${ios.id}"]`);
  await iosRow.getByRole('button',{name:'Edit app',exact:true}).click();
  await dialog.waitFor();
  await dialog.getByText('Inferred from the release name',{exact:true}).waitFor();
  await dialog.getByRole('textbox',{name:'App name',exact:true}).fill('Mobile iOS');
  await dialog.getByRole('textbox',{name:'Git repository',exact:true}).fill('https://github.com/example/mobile');
  await dialog.getByRole('button',{name:'Save',exact:true}).click(); await dialog.waitFor({state:'hidden'});
  await iosRow.getByRole('link',{name:'https://github.com/example/mobile'}).waitFor();
  assert.equal(await iosRow.getByText('Mobile iOS',{exact:true}).count(),1);
  for(const theme of ['light','dark']) for(const width of [320,1440]) {
    await page.evaluate(theme=>localStorage.setItem('sentori-theme',theme),theme);
    await page.setViewportSize({width,height:1100}); await page.reload(); await iosRow.waitFor();
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth+1),false);
    await card(multiName).scrollIntoViewIfNeeded();
    await page.screenshot({path:out+`${engine}-apps-${theme}-${width}.png`});
  }
  await iosRow.getByRole('button',{name:'Mobile 1.0',exact:true}).click();
  await page.waitForURL('**/releases?project='+multi.id+'&app='+ios.id);
  await page.getByRole('heading',{name:'Releases',exact:true}).waitFor();
  await page.getByRole('button',{name:'Mobile 1.0',exact:true}).waitFor();
  await page.getByRole('button',{name:'Mobile 1.0',exact:true}).click();
  await page.getByRole('region',{name:'Mobile 1.0',exact:true}).waitFor();
  const raw=page.locator('.release-identifier');assert.equal(await raw.getAttribute('open'),null);
  await raw.locator('summary').click();await raw.getByText('mobile@1.0',{exact:true}).waitFor();
  await page.getByRole('button',{name:'Back to list',exact:true}).click();

  assert.equal(await page.getByRole('button',{name:'mobile 2.0',exact:true}).count(),0);
  assert.equal(await page.getByRole('button',{name:'storefront 3.0',exact:true}).count(),0);
  await page.getByRole('combobox',{name:'Apps',exact:true}).selectOption(apps.find(app=>app.platform==='android').id);
  await page.getByRole('button',{name:'mobile 2.0',exact:true}).waitFor();
  assert.equal(await page.getByRole('button',{name:'Mobile 1.0',exact:true}).count(),0);
  await page.goto(origin+'/projects'); await card(multiName).waitFor();
  await activeDocument.locator('.grid[aria-busy=false]').waitFor();
  await card(multiName).getByRole('button',{name:'Add app',exact:true}).click();
  await dialog.waitFor();
  await dialog.getByRole('textbox',{name:'App name',exact:true}).fill('API service');
  await dialog.getByRole('combobox').selectOption('webapi');
  await dialog.getByRole('textbox',{name:'App identifier',exact:true}).fill('api-service');
  await dialog.getByRole('button',{name:'Save',exact:true}).click();await dialog.waitFor({state:'hidden'});
  const configured=(await call(`/admin/api/projects/${multi.id}/apps`)).apps.find(app=>app.identifier==='api-service');
  assert.ok(configured && configured.platform==='javascript' && configured.kind==='webapi');
  const apiRow=card(multiName).locator(`[data-app-id="${configured.id}"]`);
  await apiRow.getByText('Web API',{exact:true}).waitFor();
  assert.equal(await card(multiName).locator('[data-app-count]').textContent(),'4');
  writeFileSync(out+'results.json',JSON.stringify({results},null,2));
  console.log('PASS',engine,'project CRUD, stale editor recovery, typed delete, nullable platform, project scopes, member UI and password change');
});
console.log(`PASS ${results.length} project and dialog layouts`);
