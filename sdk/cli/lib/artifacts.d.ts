export type ArtifactRow = {
    kind: string;
    name: string;
    /** The 32-hex debug id, for dSYM slices. `null` for map files. */
    debugId: null | string;
    contentHash: string;
    sizeBytes: number;
    createdAt: string;
    /** Did the server parse it? `null` on artifacts uploaded before
     *  the check existed — "never looked at", not "looked at and
     *  fine". `false` means it stored and will symbolicate nothing. */
    usable?: boolean | null;
};
export type ArtifactsResponse = {
    release: string;
    /** False when the server has never heard this release name at all
     *  — a typo in `--release` and an un-uploaded release look the
     *  same otherwise, and only one of them is fixed by uploading. */
    known: boolean;
    kinds: Record<string, number>;
    missing: string[];
    artifacts: ArtifactRow[];
};
export declare function fetchArtifacts(opts: {
    apiUrl: string;
    release: string;
    token: string;
}): Promise<ArtifactsResponse>;
//# sourceMappingURL=artifacts.d.ts.map