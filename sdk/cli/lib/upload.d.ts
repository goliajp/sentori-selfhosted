export type UploadOpts = {
    apiUrl: string;
    token: string;
    release: string;
    kind: 'dsym' | 'proguard' | 'sourcemap' | 'srcbundle';
    path: string;
    /** Override the stored artifact name (defaults to the filename). */
    name?: string;
};
/** Gzip a payload for the wire (pass-through if it already is gzip),
 *  renaming `foo` → `foo.gz` to match. Shared by every artifact
 *  upload path so dSYM/mapping and sourcemap cannot drift. */
export declare function gzipForWire(bytes: Buffer | Uint8Array, name: string): {
    wire: Uint8Array;
    wireName: string;
};
/** Exact-range copy into a standalone ArrayBuffer. `wire.buffer` is
 *  NOT safe here: Node pools small Buffers, so the backing buffer can
 *  be 16 KB of pool (offset view) — a Blob built on it would ship
 *  garbage bytes around the payload. */
export declare function wireArrayBuffer(wire: Uint8Array): ArrayBuffer;
/** What the server said about the file it just stored.
 *
 *  `usable: false` means it landed and will symbolicate nothing —
 *  a Hermes bundle passed off as a source map, a zip where a Mach-O
 *  was expected. The server has answered this since 2.15.0 and its
 *  own comment says "the response says so"; nothing read it, so the
 *  first anyone heard was a release page months later with the file
 *  sitting under a green light.
 *
 *  Warned, never thrown. An artifact upload may not fail a
 *  customer's build — see the zero-cost rule — and this one did
 *  store. `--strict` is where a caller opts into a hard stop.
 */
export type UploadVerdict = {
    id: string;
    /** `null` for kinds the server does not parse ahead of time. */
    usable?: boolean | null;
    /** What to upload instead. Present only when `usable === false`. */
    hint?: null | string;
};
/** Say it once, at the only place every upload passes through. */
export declare function warnIfUnusable(name: string, v: UploadVerdict): boolean;
/**
 * Turn one path into the source maps under it, each with the name the
 * server should store.
 *
 * A file keeps its basename. A **directory** is walked, and every map
 * under it keeps its path *relative to that directory* — so
 * `dist/pages/cart/index.js.map` is stored as
 * `pages/cart/index.js.map` and not as `index.js.map`.
 *
 * That difference is the whole feature. The server matches a frame to
 * a map by how many trailing path segments they share, precisely
 * because a WeChat mini program names every page's entry `index.js`
 * and a web build names every route chunk after its route. Sending
 * only the basename threw that away on the client, so the matching
 * could never see more than a filename however carefully it was
 * written.
 *
 * Passing a directory used to read the directory as a file and throw
 * `EISDIR`, while `docs/getting-started/web.md` told readers to pass
 * `./dist`.
 */
export declare function expandSourcemapPaths(path: string): {
    path: string;
    name: string;
}[];
export declare function uploadArtifact(opts: UploadOpts): Promise<UploadVerdict>;
//# sourceMappingURL=upload.d.ts.map