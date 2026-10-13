import assert from 'node:assert/strict';
import {readFileSync,readdirSync} from 'node:fs';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import ts from 'typescript';
const root=fileURLToPath(new URL('../',import.meta.url));
const files=dir=>readdirSync(dir,{withFileTypes:true}).flatMap(e=>e.isDirectory()?files(join(dir,e.name)):[join(dir,e.name)]);
function rawControls(source){
 const ast=ts.createSourceFile('surface.tsx',source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX),bad=[];
 function visit(node){if((ts.isJsxOpeningElement(node)||ts.isJsxSelfClosingElement(node))&&['button','input','select','textarea'].includes(node.tagName.getText(ast)))bad.push(node.tagName.getText(ast));ts.forEachChild(node,visit);}visit(ast);return bad;
}
assert.deepEqual(rawControls('<button onClick={() => true}><input/><select/><textarea/></button>'),['button','input','select','textarea']);
assert.deepEqual(rawControls('<Action><Input/><NativeSelect/><Textarea/></Action>'),[]);
for(const dir of ['src','shared/documents','shared/site'])for(const file of files(join(root,dir)).filter(f=>f.endsWith('.tsx')))assert.deepEqual(rawControls(readFileSync(file,'utf8')),[],`${file}: product surfaces must compose shared controls`);
const literal=/(?:#[\da-f]{3,8}\b|\b(?:rgb|hsl)a?\s*\()/i;
assert.ok(literal.test('background: #fff'));assert.ok(literal.test('color: rgba(0,0,0,.5)'));assert.ok(!literal.test('color: var(--ui-ink)'));
for(const dir of ['src','shared/design','shared/documents','shared/site'])for(const file of files(join(root,dir)).filter(f=>f.endsWith('.css')&&!f.endsWith('/tokens.css')))assert.ok(!literal.test(readFileSync(file,'utf8')),`${file}: paint belongs to theme tokens`);
for(const file of ['shared/design/controls.css','vis/src/styles.css'])assert.match(readFileSync(join(root,file),'utf8'),/@import ["'][^"']*choices\.css["']/);
const choice=readFileSync(join(root,'shared/design/choices.css'),'utf8');assert.match(choice,/:indeterminate/);assert.match(choice,/forced-colors/);assert.match(choice,/\.choice-control\s*\{/);
for(const file of ['shared/design/controls.css','vis/src/styles.css'])assert.doesNotMatch(readFileSync(join(root,file),'utf8'),/^\.choice-control\s*\{/m,'Choice geometry has one style owner');
console.log('PASS shared production controls, choice ownership and token-only theme paint');
