// The resolved configuration, and the gate every verb checks.

import type { InitConfig, WireEvent } from '@goliapkg/sentori-core'

export type ResolvedConfig = {
  token: string
  ingestUrl: string
  release: string
  environment: string
  enabled: boolean
  backendHealthUrl?: string
  beforeSend?: (e: WireEvent) => null | WireEvent
  detect: {
    /** `wx.onError` and `wx.onUnhandledRejection`. */
    uncaught: boolean
    /** `wx.onPageNotFound`, `onMemoryWarning`, `onLazyLoadError` —
     *  three failures that are specific to this platform and that no
     *  other SDK has an equivalent for. */
    platform: boolean
    /** `wx.request` breadcrumbs, via a wrapper. */
    breadcrumbs: boolean
  }
}

export type WeappInitConfig = InitConfig & {
  detect?: {
    uncaught?: boolean
    platform?: boolean
    breadcrumbs?: boolean
  }
}

let _config: null | ResolvedConfig = null

export const setConfig = (c: ResolvedConfig): void => {
  _config = c
}

export const getConfig = (): null | ResolvedConfig => _config

export const __resetForTests = (): void => {
  _config = null
}
