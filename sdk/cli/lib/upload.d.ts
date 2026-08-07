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
export declare function uploadArtifact(opts: UploadOpts): Promise<{
    id: string;
}>;
//# sourceMappingURL=upload.d.ts.map