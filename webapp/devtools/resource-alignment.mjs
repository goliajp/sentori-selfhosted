import assert from 'node:assert/strict';

// Inspect painted pixels. DOM text ranges report the font box, not glyph ink.
export async function measureResourceAlignment(page) {
  await page.evaluate(() => document.fonts.ready);
  const sample = await page.evaluate(() => {
    const rect = el => { const r = el.getBoundingClientRect(); return { x:r.x, y:r.y, width:r.width, height:r.height }; };
    return { width: innerWidth, rows: [...document.querySelectorAll('.workbench-tree a:has(.resource-label)')]
      .filter(el => {
        if (!el.checkVisibility()) return false;
        const r=el.getBoundingClientRect(),x=r.x+r.width/2;
        return r.bottom<=innerHeight&&r.top>=0&&el.contains(document.elementFromPoint(x,r.top+.5))&&el.contains(document.elementFromPoint(x,r.bottom-.5));
      })
      .map(el => ({ row:rect(el), marker:rect(el.querySelector('.resource-label-marker')),
        text:[...el.querySelectorAll('.resource-text')].map(text => ({ box:rect(text), label:text.textContent, baseline:rect(text.querySelector('.resource-text-baseline')).y,
          shift:text.querySelector('.resource-text-line').style.getPropertyValue('--resource-text-offset'),font:getComputedStyle(text).font })) })) };
  });
  if (!sample.rows.length) return [];
  const png = (await page.screenshot()).toString('base64');
  const rows = await page.evaluate(async ({ sample, png }) => {
    const image = await createImageBitmap(new Blob([Uint8Array.from(atob(png), char => char.charCodeAt(0))], { type:'image/png' }));
    const canvas = document.createElement('canvas'); canvas.width=image.width; canvas.height=image.height;
    const ctx=canvas.getContext('2d'); ctx.drawImage(image,0,0); image.close();
    const pixels=ctx.getImageData(0,0,canvas.width,canvas.height).data, scale=canvas.width/sample.width;
    const rgb=(x,y)=>Array.from(pixels.slice((y*canvas.width+x)*4,(y*canvas.width+x)*4+3));
    return sample.rows.map(({row,marker,text})=>{
      const background=rgb(Math.floor((row.x+5)*scale),Math.floor((row.y+row.height/2)*scale));
      const center=row.y+row.height/2;
      return {center, markerOffset:marker.y+marker.height/2-center, text:text.map(({box,label,baseline,shift,font})=>{
        const ys=[];
        for(let y=Math.ceil(box.y*scale);y<Math.floor((box.y+box.height)*scale);y++) {
          for(let x=Math.ceil(box.x*scale);x<Math.floor((box.x+box.width)*scale);x++) {
            if(Math.max(...rgb(x,y).map((value,index)=>Math.abs(value-background[index])))>45){ys.push(y);break;}
          }
        }
        return {label,baseline,shift,font, inkTop:ys.length?Math.min(...ys)/scale:null, inkBottom:ys.length?(Math.max(...ys)+1)/scale:null,
          offset:ys.length?(Math.min(...ys)+Math.max(...ys)+1)/(2*scale)-center:null};
      })};
    });
  }, { sample,png });
  return rows;
}

export async function checkResourceAlignment(page, context = {}) {
  const rows = await measureResourceAlignment(page);
  for(const row of rows) {
    assert.ok(Math.abs(row.markerOffset)<.75,JSON.stringify({context,row}));
    for(const text of row.text) assert.ok(text.offset!==null&&Math.abs(text.offset)<.75,JSON.stringify({context,row,text}));
  }
  return rows;
}
