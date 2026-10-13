import assert from 'node:assert/strict';
import {readFileSync,readdirSync} from 'node:fs';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
import ts from 'typescript';
const read=p=>readFileSync(new URL('../'+p,import.meta.url),'utf8');
assert.match(read('src/components/ui.tsx'),/<NativeSelect\b/);
assert.match(read('shared/design/controls.tsx'),/className={`control native-select/);
assert.match(read('shared/design/controls.css'),/\.native-select[^}]*appearance: none/);
for(const theme of ['light','dark'])assert.match(read('shared/design/tokens.css'),new RegExp(`color-scheme: ${theme};\\s*--ui-select-chevron:`));
const files=dir=>readdirSync(dir,{withFileTypes:true}).flatMap(e=>e.isDirectory()?files(join(dir,e.name)):[join(dir,e.name)]);
function rawSelect(text){
 const ast=ts.createSourceFile('surface.tsx',text,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);let found=false;
 function visit(node){if((ts.isJsxOpeningElement(node)||ts.isJsxSelfClosingElement(node))&&node.tagName.getText(ast)==='select')found=true;ts.forEachChild(node,visit);}visit(ast);return found;
}
assert.ok(rawSelect('<select onChange={() => true}/>'));assert.ok(!rawSelect('<Select/>'));
for(const file of files(fileURLToPath(new URL('../src',import.meta.url))).filter(f=>f.endsWith('.tsx')))assert.ok(!rawSelect(readFileSync(file,'utf8')),`${file}: select must compose the shared NativeSelect`);
console.log('PASS native select appearance and chevron palette share one owner');
