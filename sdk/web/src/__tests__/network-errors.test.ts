import { afterEach, expect, test } from 'bun:test'
import { installNetworkErrors } from '../network-errors.js'
const original={window:globalThis.window,location:globalThis.location,XMLHttpRequest:globalThis.XMLHttpRequest}
let stop:(()=>void)|undefined
class Xhr extends EventTarget { status=0; open(..._args:unknown[]){} send(..._args:unknown[]){} }
afterEach(()=>{stop?.();stop=undefined;Object.assign(globalThis,original)})
function setup(fetcher:typeof fetch) {
  Object.assign(globalThis,{window:{fetch:fetcher},location:{href:'https://site.test/',origin:'https://site.test'},XMLHttpRequest:Xhr})
  const reports:unknown[][]=[];stop=installNetworkErrors('https://collector.test',(...args)=>reports.push(args));return reports
}
const tick=()=>new Promise(resolve=>setTimeout(resolve,0))
test('fetch preserves its response and original synchronous failure while reporting only policy failures',async()=>{
  const response=Promise.resolve(new Response('',{status:503}))
  const reports=setup((()=>response) as unknown as typeof fetch)
  expect(await window.fetch('https://site.test/orders/123?secret=1')).toBe(await response)
  await tick();expect(reports).toEqual([['http','/orders/:id',503]])
  const prior=window.fetch;stop?.();expect(window.fetch).not.toBe(prior)
  const failure=new TypeError('host failure')
  setup((()=>{throw failure}) as unknown as typeof fetch)
  expect(()=>window.fetch('/test')).toThrow(failure)
})
test('aborts, 4xx, opaque responses and Sentori transport never generate a recursive error',async()=>{
  for(const response of [new Response('',{status:404}),Response.error(),new Response('',{status:500})]) {
    const reports=setup((()=>Promise.resolve(response)) as unknown as typeof fetch)
    await window.fetch(response.status===500?'https://collector.test/v1/events':'https://site.test/a');await tick();expect(reports).toEqual([]);stop?.()
  }
  const reports=setup((()=>Promise.reject(new DOMException('cancel','AbortError'))) as unknown as typeof fetch)
  await window.fetch('https://site.test/').catch(()=>{});await tick();expect(reports).toEqual([])
})
test('XHR observes 5xx and connectivity, ignores cancellation, caps reports and restores owned methods',()=>{
  const reports=setup((()=>Promise.resolve(new Response(''))) as unknown as typeof fetch)
  for(let i=0;i<15;i++){const xhr=new XMLHttpRequest();xhr.open('GET','/a');xhr.send();Object.assign(xhr,{status:500});xhr.dispatchEvent(new Event('loadend'))}
  expect(reports).toHaveLength(10)
  stop?.();const next=setup((()=>Promise.resolve(new Response(''))) as unknown as typeof fetch)
  const xhr=new XMLHttpRequest();xhr.open('GET','/abort');xhr.send();xhr.dispatchEvent(new Event('abort'));xhr.dispatchEvent(new Event('error'));xhr.dispatchEvent(new Event('loadend'));expect(next).toEqual([])
  const failed=new XMLHttpRequest();failed.open('GET','/offline');failed.send();failed.dispatchEvent(new Event('error'));failed.dispatchEvent(new Event('loadend'));expect(next).toEqual([['connectivity','/offline',undefined]])
})
