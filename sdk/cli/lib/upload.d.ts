export type UploadOptions = {
    release: string;
    token: string;
    /** Sentori API base, e.g. https://sentori.golia.jp or your host. */
    apiUrl: string;
    /** Files or directories. Directories are scanned one level deep. */
    paths: string[];
    dryRun?: boolean;
};
export type UploadResult = {
    files: string[];
    uploaded?: number;
    artifacts?: {
        kind: string;
        name: string;
    }[];
};
/** Resolve `paths` (files or dirs) to a deduped list of files to upload.
 *  A directory contributes its top-level `.map` / `.js` / `.bundle` /
 *  `.hbc` files; a file given explicitly is taken as-is (even if its
 *  extension isn't in the list — the caller asked for it). */
export declare function collectFiles(paths: string[]): Promise<string[]>;
export declare function uploadSourcemaps(opts: UploadOptions): Promise<UploadResult>;
//# sourceMappingURL=upload.d.ts.map