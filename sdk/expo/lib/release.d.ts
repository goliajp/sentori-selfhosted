import type { ExpoApplicationLike } from './types.js';
/**
 * Build a `slug@version+build` release string from expo-application.
 * Returns `undefined` when the module isn't available so the caller
 * can fall back to a manually-supplied release.
 */
export declare function deriveRelease(app: ExpoApplicationLike | undefined): string | undefined;
//# sourceMappingURL=release.d.ts.map