// The `wx` surface this SDK uses, and nothing more.
//
// Declared rather than depended on: `miniprogram-api-typings` is a
// large package that pins a base-library version, and what is needed
// here is eight functions. Narrow on purpose — every name below is one
// this SDK actually calls, so the list doubles as the answer to "what
// does Sentori touch in my mini program".
//
// `lib: ES2018` in the tsconfig for the same reason: the WeChat base
// library's JavaScript engine is not a browser's, and targeting DOM
// would let a DOM API compile here and fail on a phone.

export type WxRequestOptions = {
  url: string
  method?: string
  data?: unknown
  header?: Record<string, string>
  timeout?: number
  success?: (res: { statusCode: number; data: unknown; header?: Record<string, string> }) => void
  fail?: (err: { errMsg?: string }) => void
  complete?: () => void
}

export type WxLike = {
  request(options: WxRequestOptions): unknown
  getStorageSync(key: string): unknown
  setStorageSync(key: string, value: unknown): void
  removeStorageSync(key: string): void
  getSystemInfoSync?: () => {
    brand?: string
    model?: string
    system?: string
    platform?: string
    language?: string
    windowWidth?: number
    windowHeight?: number
    pixelRatio?: number
    SDKVersion?: string
  }
  onError?: (fn: (msg: string) => void) => void
  onUnhandledRejection?: (fn: (res: { reason: unknown }) => void) => void
  onPageNotFound?: (fn: (res: { path: string; query?: unknown }) => void) => void
  onMemoryWarning?: (fn: (res: { level?: number }) => void) => void
  onLazyLoadError?: (fn: (res: { type?: string; subpackage?: unknown; errMsg?: string }) => void) => void
}

declare const wx: WxLike | undefined

let injected: null | WxLike = null

/**
 * Point the SDK at a `wx` that is not the global one.
 *
 * A mini program has the global and needs none of this. It exists for
 * the harness that drives this SDK against a real server outside the
 * WeChat runtime — the only way to check the wire without a phone —
 * and for tests. Named for what it does rather than `__forTests`,
 * because the live gate is not a test and calling it one would make
 * the gate look optional.
 */
export const setWxHost = (w: null | WxLike): void => {
  injected = w
}

export const getWx = (): null | WxLike => {
  if (injected) return injected
  try {
    return typeof wx === 'undefined' ? null : wx
  } catch {
    return null
  }
}
