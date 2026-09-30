// What the browser is, as far as it will say.
//
// No fingerprinting: everything here is something the page already
// tells every server it talks to, and nothing is combined to identify
// a person. `userAgentData` when the browser has it, because the UA
// string is frozen in Chromium and increasingly a lie.

import type { WirePayload } from '@goliapkg/sentori-core'

type UADataLike = {
  platform?: string
  mobile?: boolean
  brands?: { brand: string; version: string }[]
}

export const collectDevice = (): undefined | WirePayload['device'] => {
  if (typeof navigator === 'undefined') return undefined
  const nav = navigator as Navigator & { userAgentData?: UADataLike }
  const ua = nav.userAgentData
  const screen = typeof window !== 'undefined' ? window.screen : undefined
  return {
    os: ua?.platform ?? osFromUserAgent(nav.userAgent),
    osVersion: undefined,
    model: ua?.mobile ? 'mobile' : 'desktop',
    // `devicePixelRatio` and the viewport, because a layout bug that
    // only happens at one width is otherwise unreproducible from the
    // report.
    screen:
      screen && typeof window !== 'undefined'
        ? {
            width: window.innerWidth,
            height: window.innerHeight,
            scale: window.devicePixelRatio,
          }
        : undefined,
    locale: nav.language,
  }
}

/** A coarse family, not a version. The UA string's version numbers are
 *  frozen or spoofed often enough that reporting them invites a reader
 *  to trust a number nobody should. */
const osFromUserAgent = (ua: string): string => {
  if (/Android/i.test(ua)) return 'Android'
  if (/iPhone|iPad|iPod/i.test(ua)) return 'iOS'
  if (/Mac OS X/i.test(ua)) return 'macOS'
  if (/Windows/i.test(ua)) return 'Windows'
  if (/Linux/i.test(ua)) return 'Linux'
  return 'unknown'
}
