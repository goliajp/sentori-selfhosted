import { describe, expect, test } from 'bun:test';
import { renderArticle } from '../shared/documents/render';
const href = (_page:string, params:Record<string,string> = {}) => `/docs/${params.topic ?? ''}?version=${params.version ?? 'current'}${params.anchor ? `#${params.anchor}` : ''}`;
function render(markdown:string) { return renderArticle({slug:'getting-started/web',title:'Web',markdown},'v4.0.0',{href,omitTitle:true}); }
describe('documentation navigation and source fidelity',()=>{
 test('relative documentation links and source links retain the selected release',()=>{
  const {html}=render('[Protocol](../protocol.md#events) [SDK](../../sdk/web/README.md)');
  expect(html).toContain('/docs/protocol?version=v4.0.0#events');
  expect(html).toContain('https://github.com/goliajp/sentori-selfhosted/blob/v4.0.0/sdk/web/README.md');
 });
 test('external protocol-relative links remain external and unsafe schemes are discarded',()=>{
  const {html}=render('[External](//example.com/docs/a) [Bad](javascript:alert%281%29)');
  expect(html).toContain('href="https://example.com/docs/a"');expect(html).not.toContain('javascript:');
 });
 test('duplicate headings get independent links without exposing the duplicate page title',()=>{
  const result=render('# Web\n\n## Install\n\nFirst.\n\n## Install\n\nSecond.');
  expect(result.html).not.toContain('<h1');expect(result.headings.map(h=>h.id)).toEqual(['install','install-1']);
  expect(result.html).toContain('/docs/getting-started/web?version=v4.0.0#install-1');
 });
 test('fenced code keeps its source text while tables scroll independently',()=>{
  const result=render('```ts\nconst a = "<unsafe>";\n\nrun(a);\n```\n\n| Name | Value |\n| --- | --- |\n| a | b |');
  expect(result.codes).toEqual([{code:'const a = "<unsafe>";\n\nrun(a);',language:'ts'}]);
  expect(result.html).toContain('class="document-table-scroll"');expect(result.html).not.toContain('<unsafe>');
 });
});
