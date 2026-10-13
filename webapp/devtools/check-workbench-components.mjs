import assert from 'node:assert/strict';
import {readFileSync} from 'node:fs';
import ts from 'typescript';

function sharedComponent(file, name, source) {
  const text=readFileSync(new URL('../'+file,import.meta.url),'utf8');
  const ast=ts.createSourceFile(file,text,ts.ScriptTarget.Latest,true,ts.ScriptKind.TSX);
  let alias,used=false;
  for(const statement of ast.statements)if(ts.isImportDeclaration(statement)&&statement.moduleSpecifier.text===source){
    const bindings=statement.importClause?.namedBindings;
    if(bindings&&ts.isNamedImports(bindings))for(const binding of bindings.elements)if((binding.propertyName??binding.name).text===name)alias=binding.name.text;
  }
  assert.ok(alias,`${file} must import ${name} from its shared owner`);
  function visit(node){if((ts.isJsxOpeningElement(node)||ts.isJsxSelfClosingElement(node))&&node.tagName.getText(ast)===alias)used=true;ts.forEachChild(node,visit);}
  visit(ast);assert.ok(used,`${file} must render ${name}`);
}
sharedComponent('src/App.tsx','Workbench','./components/Workbench');
sharedComponent('src/components/Workbench.tsx','WorkbenchLayout','../../shared/design/WorkbenchLayout');
sharedComponent('src/components/Workbench.tsx','DocumentTabs','../../shared/design/DocumentTabs');
sharedComponent('src/components/Workbench.tsx','Picker','../../shared/design/Picker');
sharedComponent('src/components/Workbench.tsx','ResourceLabel','../../shared/design/ResourceLabel');
sharedComponent('vis/src/components/Workbench.tsx','ResourceLabel','../../../shared/design/ResourceLabel');
sharedComponent('vis/src/components/Workbench.tsx','WorkbenchLayout','../../../shared/design/WorkbenchLayout');
sharedComponent('vis/src/components/DocumentTabs.tsx','DocumentTabs','../../../shared/design/DocumentTabs');
sharedComponent('vis/src/components/Selection.tsx','Picker','../../../shared/design/Picker');
sharedComponent('src/pages/Releases.tsx','StatusLabel','../../shared/design/StatusLabel');
sharedComponent('vis/src/pages/Operations.tsx','StatusLabel','../../../shared/design/StatusLabel');
sharedComponent('vis/src/components/DataChart.tsx','StatusLabel','../../../shared/design/StatusLabel');
for(const [file,style] of [['src/styles/index.css','workbench.css'],['src/styles/index.css','selection.css'],['vis/src/workbench.css','workbench.css'],['vis/src/selection.css','selection.css']])assert.ok(readFileSync(new URL('../'+file,import.meta.url),'utf8').includes(`shared/design/${style}`),`${file} must load shared ${style}`);
console.log('PASS: production and preview render the same workbench, document tabs and picker owners');
