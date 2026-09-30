// Uncaught errors and unhandled rejections.
//
// Installed with `addEventListener`, never by assigning
// `window.onerror`. Assignment replaces whatever the host already had
// — their own handler, another vendor's, a framework's — and the
// symptom is "our error reporting stopped when we added Sentori",
// which is the worst possible thing for this product to cause.
//
// Neither listener calls `preventDefault`. The error still reaches the
// console and every other listener; we are watching, not intercepting.

import { coerceError } from '@goliapkg/sentori-core'

import { error as reportError } from '../verbs.js'

let installed = false
let onError: null | ((e: ErrorEvent) => void) = null
let onRejection: null | ((e: PromiseRejectionEvent) => void) = null

export const installUncaughtHandlers = (): void => {
  if (installed || typeof window === 'undefined') return
  installed = true

  onError = (e: ErrorEvent): void => {
    // `e.error` is the Error object when there is one. Cross-origin
    // scripts without `crossorigin="anonymous"` give the browser's
    // opaque "Script error." with no stack and no file — reported
    // anyway, under a name that says why it is useless, because
    // silently dropping them makes an error budget look healthy.
    const err = e.error ?? new Error(e.message || 'Script error.')
    reportError(err, {
      handled: false,
      ...(e.filename ? { filename: e.filename } : {}),
      ...(e.error ? {} : { opaque: 'cross-origin script without crossorigin=anonymous' }),
    })
  }

  onRejection = (e: PromiseRejectionEvent): void => {
    // A rejection value is frequently not an Error — a string, a
    // fetch Response, a plain object. `coerceError` is the same
    // normaliser the other SDKs use, so one rejected string
    // fingerprints the same way everywhere.
    reportError(coerceError(e.reason), { handled: false, source: 'rejection' })
  }

  window.addEventListener('error', onError)
  window.addEventListener('unhandledrejection', onRejection)
}

export const uninstallUncaughtHandlers = (): void => {
  if (typeof window === 'undefined') return
  if (onError) window.removeEventListener('error', onError)
  if (onRejection) window.removeEventListener('unhandledrejection', onRejection)
  onError = null
  onRejection = null
  installed = false
}
