import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { lightState, lightColour } from '../src/lib/release-lights.ts';
const cases = [
  [{count:0},'absent'],
  [{},'unchecked'],
  [{count:1,usable:true},'valid'],
  [{count:1,broken:true},'invalid'],
  [{count:2,usable:true,broken:true},'partial'],
  [{count:2,unchecked:true,broken:true},'partial'],
  [{count:2,usable:true,unchecked:true},'unchecked'],
  [{count:1,unchecked:true},'unchecked'],
];
for(const [input,want] of cases)assert.equal(lightState(input),want);
assert.notEqual(lightColour('valid'),lightColour('partial'));
assert.notEqual(lightColour('absent'),lightColour('invalid'));
const page=readFileSync(new URL('../src/pages/Releases.tsx',import.meta.url),'utf8');
for(const state of ['valid','partial','invalid','absent','unchecked'])assert.ok(page.includes(`['${state}',`),`legend must explain ${state}`);
assert.ok(!page.includes("kind==='sourcemap'||"),'source maps are not universally required');
assert.ok(page.includes('artifactLabel(kind,t)'),'labels must describe file types');
console.log(`✓ ${cases.length+9} artifact-state assertions; absent files do not claim missing platform coverage`);
