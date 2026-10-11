// Production badge wrappers must compose the shared primitive; layout checks
// separately measure actual text and control geometry in the browser.
import assert from 'node:assert/strict';
import ts from 'typescript';
import {readFileSync,readdirSync} from 'node:fs';
import {join} from 'node:path';
import {fileURLToPath} from 'node:url';
const root=fileURLToPath(new URL('../',import.meta.url));
function inspect(source,file) {
  const ast=ts.createSourceFile(file,source,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  const primitives=new Set();
  for(const node of ast.statements)if(ts.isImportDeclaration(node)&&node.moduleSpecifier.text.endsWith('/design/controls')) {
    for(const binding of node.importClause?.namedBindings?.elements??[])if((binding.propertyName??binding.name).text==='Badge')primitives.add(binding.name.text);
  }
  const failures=[];
  function visit(node) {
    const name=ts.isFunctionDeclaration(node)?node.name?.text:ts.isVariableDeclaration(node)?node.name.getText(ast):null;
    if(name?.endsWith('Badge')) {
      const body=ts.isFunctionDeclaration(node)?node.body:node.initializer;
      let shared=false;
      function check(child) {
        if((ts.isJsxSelfClosingElement(child)||ts.isJsxOpeningElement(child))&&primitives.has(child.tagName.getText(ast)))shared=true;
        ts.forEachChild(child,check);
      }
      if(body)check(body);
      if(!shared)failures.push(name);
    }
    ts.forEachChild(node,visit);
  }
  visit(ast);return failures;
}
assert.deepEqual(inspect('export function KindBadge(){return <span className="rounded">error</span>}','negative.tsx'),['KindBadge']);
assert.deepEqual(inspect('import {Badge as Tag} from "../../shared/design/controls";export const KindBadge=()=> <Tag>error</Tag>','positive.tsx'),[]);
function files(dir){return readdirSync(dir,{withFileTypes:true}).flatMap(e=>e.isDirectory()?files(join(dir,e.name)):[join(dir,e.name)]);}
for(const file of files(join(root,'src')).filter(f=>f.endsWith('.tsx')))assert.deepEqual(inspect(readFileSync(file,'utf8'),file),[],`${file}: badge wrappers must compose the shared Badge`);
for(const file of ['shared/design/controls.css','vis/src/styles.css']) {
  const css=readFileSync(join(root,file),'utf8');
  assert.match(css,/@import ["'][^"']*actions\.css["']/);
  assert.doesNotMatch(css,/\.(?:button|badge)\s*\{[^}]*\bfont-weight\s*:/,`${file}: primitive typography belongs to shared/design/actions.css`);
}
console.log('PASS shared Badge composition and single Button/Badge style owner');
