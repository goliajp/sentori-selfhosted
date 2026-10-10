import assert from 'node:assert/strict';
import { withLiveBrowsers } from './live-browser.mjs';
const mailbox = process.env.SENTORI_TEST_MAILBOX;
assert.ok(mailbox && new URL(mailbox).hostname === '127.0.0.1', 'Disposable mail sink required');
await withLiveBrowsers('recovery', async ({engine, browser, context, page, origin, password, call}) => {
  await call('/auth/login',{email:'owner@example.test',password});
  const email = `recover-${engine}@example.test`, replacement = `recovered-through-email-${engine}-123`;
  await call('/admin/api/users',{email,password});
  const oldSession = await browser.newContext({locale:'en-US'});
  try {
    await call('/auth/login',{email,password},'POST',200,oldSession.request);
    await context.clearCookies();
    await page.goto(origin + '/forgot-password'); await page.locator('.auth-form').waitFor();
    await page.locator('input[type=email]').fill(email); await page.locator('.auth-form button[type=submit]').click();
    await page.getByRole('status').waitFor();
    const generic = await page.getByRole('status').textContent(); assert.ok(generic.includes('If that address exists'));
    let mail;
    for (let attempt = 0; attempt < 50; attempt++) {
      const response = await context.request.get(mailbox+'/messages'); assert.equal(response.status(),200);
      mail = (await response.json()).find(message => message.to.includes(email)); if (mail) break;
      await new Promise(resolve => setTimeout(resolve,100));
    }
    assert.ok(mail,'Reset email must arrive over SMTP'); assert.equal(mail.subject,'Reset your Sentori password');
    const link = new URL(mail.text.match(/http:\/\/localhost:8080\/reset-password\?token=\S+/)?.[0]);
    assert.ok(link.searchParams.get('token'));
    await page.locator('input[type=email]').fill(`unknown-${engine}@example.test`); await page.locator('.auth-form button[type=submit]').click();
    await page.getByRole('status').waitFor(); assert.equal(await page.getByRole('status').textContent(),generic);
    const resetUrl = origin + link.pathname + link.search;
    await page.goto(resetUrl); await page.locator('input[type=password]').first().waitFor();
    const fields = page.locator('input[type=password]');
    await fields.nth(0).fill(replacement); await fields.nth(1).fill(replacement+'different'); await page.locator('.auth-form button[type=submit]').click();
    await page.getByRole('alert').filter({hasText:'Passwords do not match'}).waitFor(); await call('/auth/me',undefined,'GET',200,oldSession.request);
    await fields.nth(1).fill(replacement); await page.locator('.auth-form button[type=submit]').click(); await page.getByText('Password updated — sign in with your new password',{exact:true}).waitFor();
    await call('/auth/me',undefined,'GET',401,oldSession.request);
    await call('/auth/login',{email,password},'POST',401);
    await call('/auth/login',{email,password:replacement},'POST',200);
    await context.clearCookies(); await page.goto(resetUrl); await fields.nth(0).fill(replacement+'again'); await fields.nth(1).fill(replacement+'again'); await page.locator('.auth-form button[type=submit]').click();
    await page.getByRole('alert').filter({hasText:'Reset failed'}).waitFor();
    assert.equal((await (await context.request.get(mailbox+'/messages')).json()).filter(message => message.to.includes(`unknown-${engine}@example.test`)).length,0);
    console.log('PASS',engine,'real SMTP recovery link, anonymous response, confirmation validation, password replacement, existing session revocation and link reuse rejection');
  } finally {await oldSession.close();}
});
