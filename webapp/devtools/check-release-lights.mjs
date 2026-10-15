import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { lightState, lightColour, uploadedArtifactKinds } from '../src/lib/release-lights.ts';
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

assert.deepEqual(uploadedArtifactKinds({artifactSummary:[{kind:'dsym',count:8},{kind:'proguard',count:2},{kind:'srcbundle',count:1}]}), ['dsym','proguard','srcbundle']);
assert.deepEqual(uploadedArtifactKinds({artifacts:[{kind:'dsym'},{kind:'sourcemap'}]}), ['sourcemap','dsym']);
assert.deepEqual(uploadedArtifactKinds({artifactSummary:[{kind:'sourcemap',count:0}]}), []);
assert.deepEqual(uploadedArtifactKinds({artifacts:[],artifactSummary:[{kind:'sourcemap',count:2}]}), []);
assert.deepEqual(uploadedArtifactKinds({}), []);
console.log('✓ Native, mixed JavaScript/native, empty and unavailable artifact inventories');
assert.deepEqual(uploadedArtifactKinds({artifacts:null,artifactSummary:[{kind:'dsym',count:1}]}), ['dsym']);
