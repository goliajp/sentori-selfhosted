// v0.9.8 — RN SDK now owns its own BreadcrumbBuffer instance.
//
// Previous behaviour delegated to sentori-core's module-scoped
// `_global` buffer. That was fine on Node / bun (single module
// instance), but Metro's CJS/ESM interop on react-native can
// instantiate `@goliapkg/sentori-core` twice when the host app and
// the SDK both resolve it — the handler writes to one `_global` and
// `capture.ts` reads from the other, leaving every event with
// `breadcrumbs: []` (Insight 2026-05-16 report).
//
// Keeping the buffer local to the RN SDK is the smallest fix that
// guarantees a single instance regardless of how the bundler
// resolves sentori-core. addBreadcrumb / getBreadcrumbs / clearBreadcrumbs
// are now self-contained.
import { BreadcrumbBuffer, logger } from '@goliapkg/sentori-core'

import type { Breadcrumb, BreadcrumbType } from './types'

declare const __DEV__: boolean | undefined

export type AddBreadcrumbInput = {
  data: Record<string, unknown>
  timestamp?: string
  type: BreadcrumbType
}

const _local = new BreadcrumbBuffer()

export const addBreadcrumb = (input: AddBreadcrumbInput): void => {
  _local.push(input.type, input.data)
  if (input.timestamp) {
    // Override the auto-stamped `now()` with the caller's value. Rare
    // path; most callers omit timestamp.
    const last = _local.snapshot().at(-1)
    if (last) last.timestamp = input.timestamp
  }
}

export const getBreadcrumbs = (): Breadcrumb[] => _local.snapshot()

export const clearBreadcrumbs = (): void => {
  _local.clear()
}

export const __resetForTests = (): void => clearBreadcrumbs()

/** v0.9.8 — `__DEV__`-gated peek used by `captureException` to log
 *  diagnostic counts to Metro. Production builds never see this. */
export const __peekBreadcrumbCount = (): number => _local.snapshot().length

/** Surface-area helper that the SDK uses internally to drop diagnostic
 *  breadcrumbs without leaking the object-form API. Same target buffer
 *  as the public `addBreadcrumb`. */
export const addInternalBreadcrumb = (type: BreadcrumbType, data: Record<string, unknown>): void => {
  _local.push(type, data)
  logger.debug('breadcrumb', type, data)
}
