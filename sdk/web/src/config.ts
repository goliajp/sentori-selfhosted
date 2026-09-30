// The resolved configuration, and the gate every verb checks.
//
// One module owns it so that "has init run" has a single answer. A
// verb called before `init` must be a no-op rather than a throw — the
// iron rule — and that is only true if there is one place that knows.

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
    /** Uncaught errors and unhandled rejections. On by default: an
     *  error reporter that does not report uncaught errors has to be
     *  asked why it is installed. */
    uncaught: boolean
    /** Core Web Vitals. On by default — the observers are passive and
     *  the browser is measuring these anyway. */
    webVitals: boolean
    /** Clicks, navigations and requests into the signal ring. */
    breadcrumbs: boolean
  }
  /** Seconds of wireframe replay held in memory. 0 disables. */
  replaySeconds: number
  /** Wireframe session replay. OFF by default: a wireframe carries the
   *  shape of the page and the text in it, so it is the host's call,
   *  not ours. */
  replayScreens: boolean
}

export type WebInitConfig = InitConfig & {
  detect?: {
    uncaught?: boolean
    webVitals?: boolean
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
