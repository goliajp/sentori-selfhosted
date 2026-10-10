export type ReplayFrame = {
  t: number;
  mediaType: string;
  base64: string;
  /** Window logical size (pt/dp) — present from SDK native ≥ 5.4;
   *  the tap markers need it to map coordinates onto the JPEG. */
  w?: number;
  h?: number;
};

/** NDJSON to frames, one line at a time. The upload is append-only,
 *  so a half-written last line is the expected damage — and the
 *  moment it is most likely is the one this player exists for: the
 *  process died mid-flush. Mapping `JSON.parse` over the lines threw
 *  the whole recording away for one bad byte, and the page said the
 *  replay failed to load. The wireframe decoder has always been
 *  per-line tolerant; this one now matches it. */
export function decodeScreenFrames(text: string): ReplayFrame[] {
  const out: ReplayFrame[] = [];
  for (const line of text.split('\n')) {
    if (!line.trim()) continue;
    try {
      const f = JSON.parse(line) as ReplayFrame;
      if (f && typeof f.t === 'number' && Number.isFinite(f.t)
        && typeof f.base64 === 'string' && f.base64.length > 0
        && typeof f.mediaType === 'string' && /^image\/[a-z0-9.+-]+$/i.test(f.mediaType)) {
        out.push({...f, w: Number.isFinite(f.w) && f.w! > 0 ? f.w : undefined,
          h: Number.isFinite(f.h) && f.h! > 0 ? f.h : undefined});
      }
    } catch {
      /* a partial last line is normal */
    }
  }
  return out.sort((a,b)=>a.t-b.t);
}

