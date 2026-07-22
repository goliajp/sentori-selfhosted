import type { AdminUpload } from './native-artifacts.js';
export type SourceBundleUploadOptions = AdminUpload & {
    /** v1.4 W26 — optional module label so polyrepo apps (main +
     *  watch ext + share ext etc.) can upload multiple bundles per
     *  (release, platform). Empty/undefined → unlabelled single
     *  bundle (v1.3 W15 behaviour). */
    module?: string;
    /** Pre-built tar.gz archive of the project's source tree. */
    path: string;
    platform: 'android' | 'ios';
};
export type SourceBundleUploadResult = {
    contentHash: string;
    kind: string;
    sizeBytes: number;
};
export declare function uploadSourceBundle(opts: SourceBundleUploadOptions): Promise<SourceBundleUploadResult>;
/** Walk `dir`, pick platform-relevant source files, and tar.gz them
 *  into a temp file. Caller is responsible for invoking `cleanup()`.
 *  Uses the system `tar` binary; that's portable enough on macOS +
 *  Linux + WSL, which covers every realistic CI environment Sentori
 *  ships to. Native Windows CI without WSL would need to gzip the
 *  archive themselves (the pre-built-path mode still works). */
export declare function buildSourceBundleFromDir(dir: string, platform: 'android' | 'ios'): Promise<{
    cleanup: () => Promise<void>;
    path: string;
}>;
//# sourceMappingURL=source-bundle.d.ts.map