import type { InitOptions } from './types.js';
/**
 * Drop-in init for Expo apps. Reads bundleId / version / build from
 * `expo-application` (which is shipped in every Expo SDK) so the
 * caller only has to supply the token. Falls back to manual config
 * fields when expo-application isn't installed (bare RN apps), in
 * which case the caller MUST pass `release`.
 *
 *     // App.tsx
 *     import { initSentoriExpo } from '@goliapkg/sentori-expo'
 *     import * as Application from 'expo-application'
 *
 *     initSentoriExpo({
 *       application: Application,
 *       token: process.env.EXPO_PUBLIC_SENTORI_TOKEN!,
 *     })
 *
 * Why we ask the caller to import `expo-application` and pass it in,
 * instead of `import * as Application from 'expo-application'` here?
 * Bundlers (Metro / Hermes) statically include every import; if this
 * package imported expo-application directly, every consumer would
 * be forced to install it even when running in a bare-RN context.
 */
export declare function initSentoriExpo(options: InitOptions): void;
/**
 * Re-export of `deriveRelease` (defined in `./release.ts`) for
 * callers who want to use the same `slug@version+build` string outside
 * of init (e.g. as a tag, log prefix, or metric label). Lives in its
 * own module so it can be unit-tested without the SDK chain pulling
 * in `react-native`'s Flow-typed exports.
 */
export { deriveRelease } from './release.js';
export type { ExpoApplicationLike, InitOptions } from './types.js';
//# sourceMappingURL=index.d.ts.map