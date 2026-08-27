/** Resolve `react-native/scripts/compose-source-maps.js` from the
 *  current project's node_modules. Returns null if react-native isn't
 *  installed or the version doesn't ship that script. */
export declare function resolveComposeScript(fromDir?: string): null | string;
/** Compose a Metro packager source map + a Hermes source map into a
 *  single map (a temp file the caller is responsible for deleting).
 *  Throws if react-native's compose script can't be found or fails. */
export declare function composeSourceMaps(metroMap: string, hermesMap: string): string;
export type RnUploadOptions = {
    apiUrl: string;
    /** Optional bundle file (.jsbundle / .bundle) to upload alongside the map. */
    bundle?: string;
    dryRun?: boolean;
    hermesMap: string;
    metroMap: string;
    release: string;
    token: string;
};
/** Compose the Metro + Hermes maps, then upload the result (and the
 *  bundle, if given). Cleans up the temp composed map. */
export declare function reactNativeUpload(opts: RnUploadOptions): Promise<{
    files: string[];
    uploaded?: number;
}>;
//# sourceMappingURL=react-native.d.ts.map