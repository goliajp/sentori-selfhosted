import assert from 'node:assert/strict';
import {writeFileSync} from 'node:fs';
import {checkResourceAlignment} from './resource-alignment.mjs';

export async function checkHeaderControls(page) {
 const geometry=await page.locator('.studio-topbar .control-row').evaluate(row=>{
  const select=row.querySelector('select'),theme=row.querySelector('[role=radiogroup]');
  const box=el=>{const r=el.getBoundingClientRect();return {height:r.height,width:r.width,center:r.y+r.height/2};};
  return {size:parseFloat(getComputedStyle(row).getPropertyValue('--control-size')),
   layout:row.dataset.controlLayout,select:box(select),theme:box(theme),grow:getComputedStyle(select).flexGrow};
 });
 assert.equal(geometry.layout,'content');assert.equal(geometry.grow,'0');
 for(const control of [geometry.select,geometry.theme])assert.ok(Math.abs(control.height-geometry.size)<.75,JSON.stringify(geometry));
 assert.ok(Math.abs(geometry.select.center-geometry.theme.center)<.75,JSON.stringify(geometry));
 return geometry;
}

export async function checkControlSystem({page,origin,project,engine,out}) {
 const results=[];
 const visit=async path=>{await page.waitForLoadState('networkidle');await page.goto(origin+path);await page.waitForLoadState('networkidle');};
 for(const locale of ['en','zh','ja'])for(const theme of ['light','dark'])for(const width of [320,768,1440]) {
  await page.evaluate(({locale,theme})=>{localStorage.setItem('sentori_locale',locale);localStorage.setItem('sentori-theme',theme);},{locale,theme});
  await page.setViewportSize({width,height:1000});await visit(`/settings/notifications?project=${project.id}`);
  await page.locator('.workbench-document[data-active=true] .choice-control').first().waitFor();
  const geometry=await page.evaluate(()=>{
   const center=el=>{const r=el.getBoundingClientRect();return r.y+r.height/2;};
   const textCenter=el=>{const range=document.createRange();range.selectNodeContents(el);const r=range.getBoundingClientRect();return r.y+r.height/2;};
   const rail=document.querySelector('.studio-nav'),style=getComputedStyle(rail);
   return {rail:[style.paddingTop,style.paddingLeft,style.paddingRight],
    choices:[...document.querySelectorAll('.workbench-document[data-active=true] .choice-control')].map(el=>{const input=el.querySelector('input'),label=el.querySelector('.choice-label');return {offset:center(input)-textCenter(label),width:input.getBoundingClientRect().width,appearance:getComputedStyle(input).appearance};}),
    overflow:document.documentElement.scrollWidth>innerWidth+1};
  });
  assert.equal(new Set(geometry.rail).size,1,JSON.stringify(geometry));assert.equal(geometry.rail[0],'6px');
  if(width>=768)geometry.header=await checkHeaderControls(page);
  geometry.resources=await checkResourceAlignment(page,{engine,locale,theme,width});
  if(locale==='en'&&theme==='light'&&width===1440) {
   const regression=await page.addStyleTag({content:'.resource-text-line { top:4px!important; }'});
   try { await assert.rejects(checkResourceAlignment(page,{engine,regression:'shifted resource glyphs'})); }
   finally { await regression.evaluate(el=>el.remove()); }
   geometry.rejectsShiftedGlyphs=true;
   const oversized=await page.addStyleTag({content:'.studio-topbar select.control { height:48px!important; min-height:48px!important; }'});
   try { await assert.rejects(checkHeaderControls(page)); }
   finally { await oversized.evaluate(el=>el.remove()); }
   geometry.rejectsOversizedHeader=true;
  }
  assert.ok(geometry.choices.length>0);if(width===1440)assert.ok(geometry.resources.length>0);
  for(const choice of geometry.choices){assert.equal(choice.width,18);assert.equal(choice.appearance,'none');assert.ok(Math.abs(choice.offset)<.75,JSON.stringify({engine,locale,theme,choice}));}
  assert.equal(geometry.overflow,false);
  if(width===1440) {
  const links=page.locator('.studio-nav nav a');
  for(let i=0;i<await links.count();i++){
   const link=links.nth(i);if(!await link.isVisible())continue;
   await link.hover();const tip=page.getByRole('tooltip');await tip.waitFor();assert.equal(await tip.textContent(),await link.getAttribute('aria-label'));assert.equal(await link.getAttribute('title'),null);
   const box=await tip.boundingBox();assert.ok(box.x>=0&&box.y>=0&&box.x+box.width<=1440&&box.y+box.height<=1000);
   if(i===0){await tip.hover();await page.waitForTimeout(250);assert.equal(await tip.isVisible(),true);}
   await page.keyboard.press('Escape');await tip.waitFor({state:'hidden'});
  }
  await links.first().focus();await page.getByRole('tooltip').waitFor();await page.keyboard.press('Escape');await page.getByRole('tooltip').waitFor({state:'hidden'});
  await page.locator('h1').first().click();
  }
  if(width<1000)await page.locator('.workbench-document[data-active=true] .choice-control').first().scrollIntoViewIfNeeded();
  await page.screenshot({path:out+`${engine}-controls-${locale}-${theme}-${width}.png`});
  results.push({engine,locale,theme,width,geometry});
 }
 await page.evaluate(()=>{localStorage.setItem('sentori_locale','en');localStorage.setItem('sentori-theme','light');});
 await visit(`/issues?project=${project.id}`);
 writeFileSync(out+`${engine}-control-system.json`,JSON.stringify(results,null,2));
}
