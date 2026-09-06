// Phase 21 sub-D — pure release-derivation helper, intentionally
// isolated from `./index.ts` so unit tests can exercise it without
// pulling in `@goliapkg/sentori-react-native` (and its transitive
// static import of `react-native`, which Bun's test runner can't
// parse — RN ships Flow-typed re-exports). Index.ts re-exports it.

import type { ExpoApplicationLike } from './types.js'

/**
 * Build a `slug@version+build` release string from expo-application.
 * Returns `undefined` when the module isn't available so the caller
 * can fall back to a manually-supplied release.
 */
export function deriveRelease(app: ExpoApplicationLike | undefined): string | undefined {
  if (!app) return undefined
  const id = app.applicationId ?? app.nativeApplicationVersion ?? 'app'
  const version = app.nativeApplicationVersion ?? '0.0.0'
  const build = app.nativeBuildVersion ?? '0'
  return `${id}@${version}+${build}`
}
