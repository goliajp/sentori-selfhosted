export type JsonFold = { start: number; end: number; opening: '{'|'['; closing: '}'|']'; count: number };
/** Scan delimiters outside strings; display ranges retain original source line numbers. */
export function jsonFolds(source: string): JsonFold[] {
  try { JSON.parse(source); } catch { return []; }
  const stack: {offset:number;line:number;opening:'{'|'['}[] = [], ranges:JsonFold[] = [];
  let quoted = false, escaped = false, line = 0;
  for (let offset=0;offset<source.length;offset++) {
    const c = source[offset];
    if (c === '\n') line++;
    if (quoted) { if (escaped) escaped=false; else if (c==='\\') escaped=true; else if (c==='"') quoted=false; continue; }
    if (c==='"') { quoted=true; continue; }
    if (c==='{' || c==='[') stack.push({offset,line,opening:c});
    else if (c==='}' || c===']') {
      const start=stack.pop();
      if (start && line>start.line) {
        const value=JSON.parse(source.slice(start.offset,offset+1));
        ranges.push({start:start.line,end:line,opening:start.opening,closing:c,count:Array.isArray(value)?value.length:Object.keys(value).length});
      }
    }
  }
  return ranges.sort((a,b)=>a.start-b.start||b.end-a.end);
}
export function visibleJsonLines(length:number, ranges:JsonFold[], collapsed:ReadonlySet<number>) {
  const starts=new Map(ranges.map(range=>[range.start,range]));
  const result:number[]=[];
  for(let line=0;line<length;line++){result.push(line);const range=starts.get(line);if(range&&collapsed.has(line))line=range.end;}
  return result;
}
