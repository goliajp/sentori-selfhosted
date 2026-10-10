export type CodeReference = { symbol:string; line?:number; target?:{file:string;line:number}; value?:string };
const escape = (s:string)=>s.replaceAll('&','&amp;').replaceAll('<','&lt;').replaceAll('>','&gt;').replaceAll('"','&quot;');
/** Decorate identifiers in text nodes only; preserve the highlighter's escaped markup. */
export function annotateCodeHTML(html:string,line:number,references:CodeReference[],hint:string) {
  const eligible=references.map((ref,index)=>({ref,index})).filter(({ref})=>(ref.line===undefined||ref.line===line)&&/^[A-Za-z_$][\w$]*$/.test(ref.symbol));
  if(!eligible.length)return html;
  const names=new Map(eligible.map(item=>[item.ref.symbol,item]));
  const skip:boolean[]=[];
  return html.split(/(<[^>]*>)/g).map(part=>{if(part.startsWith('<')){if(/^<span\b/.test(part))skip.push(!!skip.at(-1)||/hljs-(string|comment|regexp|char)\b/.test(part));else if(/^<\/span>/.test(part))skip.pop();return part;}if(skip.at(-1))return part;return part.replace(/(?<![\w$&])[A-Za-z_$][\w$]*(?![\w$;])/g,symbol=>{
    const item=names.get(symbol);if(!item)return symbol;const {ref,index}=item;
    const title=[ref.value!==undefined?`${symbol} = ${ref.value}`:'',ref.target?`${hint}: ${ref.target.file}:${ref.target.line}`:''].filter(Boolean).join('\n');
    return `<button type="button" class="code-reference" data-code-reference="${index}" title="${escape(title)}">${escape(symbol)}</button>`;
  });}).join('');
}
