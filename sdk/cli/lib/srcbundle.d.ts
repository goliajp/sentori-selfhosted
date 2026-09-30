export type SrcBundleStats = {
    files: number;
    bytes: number;
    skipped: number;
};
/** Collect `{ relPath: content }` from the roots. */
export declare function collectSources(roots: string[]): {
    bundle: Record<string, string>;
    stats: SrcBundleStats;
};
//# sourceMappingURL=srcbundle.d.ts.map