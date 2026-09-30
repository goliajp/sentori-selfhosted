// What the phone is, as `wx.getSystemInfoSync` reports it.

import type { WirePayload } from '@goliapkg/sentori-core'

import { getWx } from './wx.js'

let cached: undefined | WirePayload['device']
let asked = false

export const collectDevice = (): undefined | WirePayload['device'] => {
  // Read once. `getSystemInfoSync` is a synchronous bridge call and
  // the answer does not change while the mini program is open, so
  // calling it per event would put a bridge round trip on the caller's
  // thread for a constant.
  if (asked) return cached
  asked = true
  const wx = getWx()
  if (!wx?.getSystemInfoSync) return undefined
  try {
    const info = wx.getSystemInfoSync()
    cached = {
      os: info.platform ?? 'weapp',
      osVersion: info.system,
      model: [info.brand, info.model].filter(Boolean).join(' ') || undefined,
      locale: info.language,
      screen:
        info.windowWidth && info.windowHeight
          ? { width: info.windowWidth, height: info.windowHeight, scale: info.pixelRatio }
          : undefined,
    }
  } catch {
    cached = undefined
  }
  return cached
}

export const __resetDeviceForTests = (): void => {
  cached = undefined
  asked = false
}
