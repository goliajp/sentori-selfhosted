import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import { withLiveBrowsers } from './live-browser.mjs';

const labels = {
  en: {title:'Reset instance',name:'Type the instance name',id:'Type the instance ID',generation:'Type the generation',password:'Current owner password',prepare:'Prepare offline reset',download:'Download reset permit',close:'Close'},
  zh: {title:'重置实例',name:'输入实例名称',id:'输入实例 ID',generation:'输入实例代次',password:'当前所有者密码',prepare:'准备离线重置',download:'下载重置许可',close:'关闭'},
  ja: {title:'インスタンスをリセット',name:'インスタンス名を入力',id:'インスタンス ID を入力',generation:'インスタンスの世代を入力',password:'現在の所有者パスワード',prepare:'オフラインリセットを準備',download:'リセット許可をダウンロード',close:'閉じる'},
};
const results = [];
await withLiveBrowsers('instance-reset', async ({engine,page,origin,out,password,call}) => {
  await call('/auth/login',{email:'owner@example.test',password},'POST',200);
  const initial = await call('/admin/api/instance');
  const history = await call('/admin/api/instance/operations');
  let layouts = 0;
  async function layout(locale,theme,phase) {
    await page.waitForLoadState('networkidle');
    await page.evaluate(() => document.fonts.ready);
    const size = await page.evaluate(() => ({width:innerWidth,scroll:document.documentElement.scrollWidth,lang:document.documentElement.lang,theme:document.documentElement.dataset.theme}));
    assert.ok(size.scroll <= size.width + 1, `${engine}/${locale}/${theme}/${size.width}/${phase}: ${JSON.stringify(size)}`);
    assert.equal(size.lang,locale); assert.equal(size.theme,theme);
    if (phase !== 'overview') {
      const bounds = await page.getByRole('dialog').boundingBox();
      assert.ok(bounds && bounds.x >= -1 && bounds.x + bounds.width <= size.width + 1);
    }
    layouts++;
  }
  for (const locale of ['en','zh','ja']) for (const theme of ['light','dark']) {
    const text = labels[locale];
    await page.goto(origin + '/self-hosted');
    // Do not abort shell initialization requests while changing the locale.
    await page.waitForLoadState('networkidle');
    await page.evaluate(({locale,theme}) => { localStorage.setItem('sentori_locale',locale); localStorage.setItem('sentori-theme',theme); }, {locale,theme});
    await page.reload();
    const launch = page.getByRole('button',{name:text.title,exact:true});
    await launch.waitFor();
    for (const width of [320,390,768,1440,2560,3840]) { await page.setViewportSize({width,height:900}); await layout(locale,theme,'overview'); }
    await page.setViewportSize({width:320,height:900});
    await launch.click();
    const dialog = page.getByRole('dialog');
    await dialog.waitFor();
    const prepare = dialog.getByRole('button',{name:text.prepare,exact:true});
    assert.equal(await prepare.isEnabled(),false);
    for (const width of [320,390,768,1440,2560,3840]) {
      await page.setViewportSize({width,height:900}); await layout(locale,theme,'form');
      if (width === 320) await page.screenshot({path:`${out}/${engine}-${locale}-${theme}-form.png`,fullPage:true});
    }
    await dialog.getByLabel(text.name,{exact:true}).fill(initial.name);
    await dialog.getByLabel(text.id,{exact:true}).fill('different-instance');
    await dialog.getByLabel(text.generation,{exact:true}).fill(String(initial.generation));
    await dialog.getByLabel(text.password,{exact:true}).fill(password);
    await dialog.getByRole('checkbox').check();
    assert.equal(await prepare.isEnabled(),false);
    await dialog.getByLabel(text.id,{exact:true}).fill(initial.id);
    assert.equal(await prepare.isEnabled(),true);
    if (locale === 'en' && theme === 'light') {
      await dialog.getByLabel(text.password,{exact:true}).fill('wrong-password');
      await prepare.click();
      await dialog.getByRole('alert').waitFor();
      assert.ok((await dialog.getByRole('alert').textContent()).includes('current password is incorrect'));
      assert.equal(await dialog.getByLabel(text.password,{exact:true}).inputValue(),'');
      await dialog.getByLabel(text.password,{exact:true}).fill(password);
    }
    const result = page.waitForResponse(response => response.url().endsWith('/admin/api/instance/reset/prepare') && response.request().method() === 'POST');
    await prepare.click();
    const response = await result;
    assert.equal(response.status(),200);
    const permit = await response.json();
    await dialog.getByRole('button',{name:text.download,exact:true}).waitFor();
    assert.equal(await dialog.locator('time').getAttribute('datetime'),permit.expiresAt);
    assert.ok((await dialog.locator('time').textContent()).includes(String(new Date(permit.expiresAt).getFullYear())));
    assert.equal(await dialog.locator('input[type=password]').count(),0);
    const download = page.waitForEvent('download');
    await dialog.getByRole('button',{name:text.download,exact:true}).click();
    const saved = await download;
    assert.equal(readFileSync(await saved.path(),'utf8').trim(),permit.permit);
    assert.match(permit.permit,/^[a-f0-9]{64}$/);
    assert.ok(saved.suggestedFilename().includes(permit.operationId));
    const commands = await dialog.locator('pre').allTextContents();
    assert.ok(commands[0].includes('--instance-id') && commands[0].includes(initial.id));
    assert.ok(commands[0].includes('--confirm-independent-storage') && commands[1].includes('finish-reset'));
    assert.equal((await dialog.textContent()).includes(permit.permit),false);
    assert.equal(await page.evaluate(token => Object.values(localStorage).some(value => value.includes(token)),permit.permit),false);
    for (const width of [320,390,768,1440,2560,3840]) {
      await page.setViewportSize({width,height:900}); await layout(locale,theme,'prepared');
      if (width === 320) await page.screenshot({path:`${out}/${engine}-${locale}-${theme}-prepared.png`,fullPage:true});
    }
    await dialog.getByRole('button',{name:text.close,exact:true}).last().click();
    await dialog.waitFor({state:'hidden'});
  }
  const after = await call('/admin/api/instance');
  assert.equal(after.id,initial.id); assert.equal(after.generation,initial.generation);
  assert.deepEqual((await call('/admin/api/instance/operations')).items,history.items);
  results.push({engine,layouts,downloadedPermit:true,wrongPassword:true,typedIdentityGate:true,noOnlineDeletion:true});
  writeFileSync(`${out}/results.json`,JSON.stringify(results,null,2));
  console.log(`PASS ${engine}: reset preparation and ${layouts} responsive layouts`);
});
