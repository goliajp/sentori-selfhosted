// One call, and the mini program is reporting.

import { safeFn, setInternalReporter, setLogLevel } from '@goliapkg/sentori-core'

import { getConfig, setConfig, type WeappInitConfig } from './config.js'
import { installNetworkSignals } from './handlers/network.js'
import { installPlatformHandlers, installUncaughtHandlers } from './handlers/uncaught.js'
import { drainOfflineQueue, startTransport } from './transport.js'
import { internalFault } from './verbs.js'

let _initialized = false

export const init = safeFn('init', (config: WeappInitConfig): void => {
  // A second call would register a second set of handlers and double
  // every event.
  if (_initialized) return
  _initialized = true

  setConfig({
    token: config.token,
    ingestUrl: config.ingestUrl.replace(/\/+$/, ''),
    release: config.release ?? '',
    environment: config.environment ?? 'production',
    enabled: true,
    backendHealthUrl: config.backendHealthUrl,
    beforeSend: config.beforeSend,
    detect: {
      uncaught: config.detect?.uncaught !== false,
      platform: config.detect?.platform !== false,
      breadcrumbs: config.detect?.breadcrumbs !== false,
    },
  })
  setLogLevel(config.logLevel ?? 'warn')
  setInternalReporter(internalFault)

  startTransport()

  const resolved = getConfig()
  if (!resolved) return
  if (resolved.detect.uncaught) installUncaughtHandlers()
  if (resolved.detect.platform) installPlatformHandlers()
  // Before anything else uses `wx.request`: the patch wraps whatever
  // is there when it runs, so installing it after the host has taken
  // its own reference means the host's copy is unwrapped.
  if (resolved.detect.breadcrumbs) installNetworkSignals(resolved.ingestUrl)

  // Whatever a previous launch could not send.
  void drainOfflineQueue()
})

export const __resetForTests = (): void => {
  _initialized = false
}
