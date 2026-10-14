export declare const SOURCE_MANIFEST = "sentori-source-manifest.json";
export declare const MAX_SOURCE_FILE_BYTES: number;
export declare const MAX_SOURCE_BYTES: number;
export type SrcBundleStats = {
    files: number;
    bytes: number;
    skipped: number;
};
export type SourceOptions = {
    root?: string;
    exclude?: string[];
    gitTag?: string;
};
export type SourceManifest = {
    version: 1;
    release: string;
    source: {
        type: 'directory' | 'git-tag';
        tag?: string;
        commit?: string;
    };
    files: {
        path: string;
        bytes: number;
        sha256: string;
    }[];
};
export type CollectedSources = {
    bundle: Record<string, string>;
    stats: SrcBundleStats;
    source: SourceManifest['source'];
};
export declare function safeSourcePath(path: string): boolean;
/** Keep every path relative to ONE root, even with several selected directories. */
export declare function collectSources(paths: string[], options?: SourceOptions): CollectedSources;
export declare function packSources(paths: string[], release: string, options?: SourceOptions): {
    bytes: Uint8Array;
    manifest: SourceManifest;
    stats: SrcBundleStats;
};
/** Validate in memory; the server independently repeats validation. */
export declare function inspectSourceArchive(bytes: Uint8Array, release?: string): {
    files: string[];
    manifest?: SourceManifest;
};
//# sourceMappingURL=srcbundle.d.ts.map