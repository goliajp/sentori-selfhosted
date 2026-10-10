import hljs from "highlight.js/lib/core";
import c from "highlight.js/lib/languages/c";
import css from "highlight.js/lib/languages/css";
import diff from "highlight.js/lib/languages/diff";
import ini from "highlight.js/lib/languages/ini";
import objectivec from "highlight.js/lib/languages/objectivec";
import sql from "highlight.js/lib/languages/sql";
import xml from "highlight.js/lib/languages/xml";
import yaml from "highlight.js/lib/languages/yaml";
import { highlightBlock, highlightLines } from "./highlight";

// Share the production parser and its lossless multiline handling. Load only
// the extra grammars required by the design-system examples.
for (const [name, grammar] of Object.entries({
  c,
  css,
  diff,
  ini,
  objectivec,
  sql,
  xml,
  yaml,
})) {
  hljs.registerLanguage(name, grammar);
}
const aliases: Record<string, string> = {
  ts: "typescript",
  tsx: "typescript",
  node: "typescript",
  js: "javascript",
  jsx: "javascript",
  sh: "bash",
  shell: "bash",
  zsh: "bash",
  shellscript: "bash",
  "swift package manager": "swift",
  gradle: "kotlin",
  kt: "kotlin",
  objc: "objectivec",
  "objective-c": "objectivec",
  "c++": "cpp",
  "c#": "csharp",
  py: "python",
  rs: "rust",
  html: "xml",
  yml: "yaml",
  toml: "ini",
  jsonc: "json",
};
export function codeGrammar(language: string): string | undefined {
  const key = language.trim().toLowerCase();
  if (["text", "plain", "plaintext"].includes(key)) return undefined;
  const grammar = Object.hasOwn(aliases, key) ? aliases[key] : key;
  return hljs.getLanguage(grammar) ? grammar : undefined;
}
export function highlightCode(code: string, language: string): string {
  return highlightBlock(code, codeGrammar(language));
}
export function highlightCodeLines(code: string, language: string): string[] {
  return highlightLines(code, codeGrammar(language));
}
