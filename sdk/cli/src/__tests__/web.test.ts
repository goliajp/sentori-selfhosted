import { afterEach, expect, test } from 'bun:test'
import { runWeb } from '../web.js'
const original = globalThis.fetch
const args = ['--token','fixture','--api-url','https://monitor.test']
const reply = (body: unknown, status=200) => new Response(JSON.stringify(body), {status,headers:{'Content-Type':'application/json'}})
afterEach(()=>{globalThis.fetch=original})
test('performance filters are encoded and token is scoped', async()=>{
  globalThis.fetch=(async(url,init)=>{
    const u=new URL(String(url)); expect(u.pathname).toBe('/api/web/performance')
    expect(u.searchParams.get('route')).toBe('/orders/:id');expect(u.searchParams.get('device')).toBe('mobile')
    expect((init?.headers as Record<string,string>).Authorization).toBe('Bearer fixture')
    return reply({visits:0})
  }) as typeof fetch
  expect(await runWeb(['performance',...args,'--app','app','--route','/orders/:id','--device','mobile'])).toBe(0)
})
test('project mismatch prevents a mutation',async()=>{
  let calls=0
  globalThis.fetch=(async()=>{calls++;return reply({project:{id:'other'}})}) as typeof fetch
  expect(await runWeb(['run',...args,'--id','check','--project','expected'])).toBe(2)
  expect(calls).toBe(1)
})
test('manual retries retain command ID and deployment trigger',async()=>{
  const bodies: unknown[]=[]
  globalThis.fetch=(async(_url,init)=>{bodies.push(JSON.parse(String(init?.body)));return reply({id:'run'})}) as typeof fetch
  for(let i=0;i<2;i++) expect(await runWeb(['run',...args,'--id','check','--command-id','same-command','--trigger','deployment'])).toBe(0)
  expect(bodies).toEqual(Array(2).fill({action:'run',commandId:'same-command',id:'check',trigger:'deployment'}))
})
test('strict distinguishes site failure, monitoring failure and successful runs',async()=>{
  for(const [status,code] of [['passed',0],['failed',1],['error',2],['cancelled',2],['running',2]] as const){
    globalThis.fetch=(async()=>reply({runs:[{id:'run',status}]})) as typeof fetch
    expect(await runWeb(['runs',...args,'--app','app','--id','run','--strict'])).toBe(code)
    expect(await runWeb(['runs',...args,'--app','app','--id','run'])).toBe(0)
  }
})
test('wait reads the exact run and returns strict failure',async()=>{
  globalThis.fetch=(async(url,init)=>{
    if(init?.method==='POST')return reply({id:'exact-run'})
    expect(new URL(String(url)).searchParams.get('id')).toBe('exact-run')
    return reply({runs:[{id:'exact-run',status:'failed'}]})
  }) as typeof fetch
  expect(await runWeb(['run',...args,'--app','app','--id','check','--wait','--strict'])).toBe(1)
})
test('invalid mutation version and unscoped strict mode never reach transport',async()=>{
  let calls=0;globalThis.fetch=(async()=>{calls++;return reply({})}) as typeof fetch
  expect(await runWeb(['delete',...args,'--id','check','--version','NaN'])).toBe(2)
  expect(await runWeb(['run',...args,'--id','check','--strict'])).toBe(2)
  expect(calls).toBe(0)
})
