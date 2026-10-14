import type { ArtifactsResponse } from './artifacts.js';
/** Read the installed package, so a release bump cannot leave a stale constant. */
export declare const CLI_VERSION: string;
/** Capabilities, not a server version comparison: backports may share versions. */
export declare function checkCompatibility(result: ArtifactsResponse, sourceZip: boolean): string;
//# sourceMappingURL=compatibility.d.ts.map