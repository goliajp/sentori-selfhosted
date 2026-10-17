import { expect, test } from 'bun:test'
import { boundedVisits, normalizeRoute, resourceSummary, type Visit } from '../performance-model.js'
function visit(id:string,revision=1):Visit { return {id,revision,startedAt:new Date().toISOString(),appIdentifier:'site',release:'1',environment:'test',route:'/',deviceClass:'desktop',navigationType:'document',sampleRate:0.1,metrics:{},missing:{INP:'no-interaction'},errorCount:0,resourceErrorCount:0,browser:'Chrome',diagnostics:{resources:[],longTasks:{count:0,totalMs:0,maxMs:0}}} }
test('normalization removes credentials, query, fragment and common identifying path segments',()=>{
  expect(normalizeRoute('https://user:secret@site.test/orders/123?token=secret#private')).toBe('/orders/:id')
  expect(normalizeRoute('/person/alice%40example.com')).toBe('/person/:id')
  expect(normalizeRoute('data:secret')).toBe('/')
})
test('healthy visits remain in a bounded UTF-8 queue; replayed revisions cannot replace newer state',()=>{
  let rows:Visit[]=[]
  for(let i=0;i<40;i++)rows=boundedVisits(rows,visit(String(i)))
  expect(rows).toHaveLength(20);expect(rows[0]?.id).toBe('20')
  rows=boundedVisits(rows,visit('39',3));rows=boundedVisits(rows,visit('39',2))
  expect(rows.find(v=>v.id==='39')?.revision).toBe(3)
  for(let i=0;i<40;i++)rows=boundedVisits(rows,{...visit('large'+i),route:'私'.repeat(240),release:'版'.repeat(200)})
  expect(new TextEncoder().encode(JSON.stringify(rows)).length).toBeLessThanOrEqual(48000)
})
test('cross-origin hidden timing is unknown, while same-origin cached zero bytes is known',()=>{
  const timing={name:'https://cdn.test/a.js?secret=1',initiatorType:'script',duration:120,transferSize:0,responseStart:0} as PerformanceResourceTiming
  expect(resourceSummary(timing,'https://site.test')).toEqual({url:'https://cdn.test/a.js',type:'script',durationMs:120,transferBytes:null,timingAvailable:false})
  expect(resourceSummary({...timing,name:'https://site.test/a.js'} as PerformanceResourceTiming,'https://site.test').transferBytes).toBe(0)
})
