export type ReplayNode = {
  x: number;
  y: number;
  w: number;
  h: number;
  kind?: string;
  text?: string;
  color?: string;
};

type WireNode = ReplayNode & { at?: number; id?: string };

export type Wireframe =
  | { ts: number; kind: 'key'; width: number; height: number; nodes: ReplayNode[] }
  | {
      ts: number;
      kind: 'delta';
      added: WireNode[];
      changed: WireNode[];
      removed: (Pick<ReplayNode, 'x' | 'y' | 'w' | 'h'> & { id?: string })[];
    };

/** NDJSON to frames. One malformed line must not cost the whole
 *  recording: the format is append-only, so a truncated tail is the
 *  expected failure rather than a corrupt file. */
export function decodeWireframes(text: string): Wireframe[] {
  const out: Wireframe[] = [];
  for (const line of text.split('\n')) {
    if (!line.trim()) continue;
    try {
      const frame = JSON.parse(line) as Wireframe;
      if (!frame || typeof frame.ts !== 'number' || !Number.isFinite(frame.ts)) continue;
      if (frame.kind === 'key' && Number.isFinite(frame.width) && frame.width > 0
        && Number.isFinite(frame.height) && frame.height > 0 && Array.isArray(frame.nodes) && frame.nodes.every(validNode)) out.push(frame);
      else if (frame.kind === 'delta' && ['added','changed','removed'].every(key => {
        const nodes = frame[key as 'added'|'changed'|'removed'];
        return Array.isArray(nodes) && nodes.every(validNode);
      })) out.push(frame);
    } catch {
      /* a partial last line is normal */
    }
  }
  let started = false;
  return out.sort((a,b)=>a.ts-b.ts).filter(frame=>{if(frame.kind==='key')started=true;return started;});
}


function validNode(value: unknown): value is ReplayNode {
  if (!value || typeof value !== 'object') return false;
  const node = value as Record<string,unknown>;
  return ['x','y','w','h'].every(key=>typeof node[key] === 'number' && Number.isFinite(node[key]))
    && (node.w as number) >= 0 && (node.h as number) >= 0
    && ['text','color','kind'].every(key=>node[key] === undefined || typeof node[key] === 'string');
}
