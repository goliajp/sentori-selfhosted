// What the person was doing before it broke.
//
// Three sources, all passive: clicks, history navigations, and
// requests. They go into the core signal ring, which is a fixed-size
// buffer of the last sixty seconds — so this costs a bounded amount of
// memory and no network at all until something goes wrong.
//
// Nothing here is a listener the host can notice. Clicks are captured
// in the capture phase and never stopped; history is patched with the
// original called first, so a patch of ours that throws cannot stop a
// navigation.

import { pushSignal, reportInternal } from '@goliapkg/sentori-core'

let installed = false
let onClick: null | ((e: MouseEvent) => void) = null

/** A short, stable description of what was clicked. Not the text: a
 *  label can be a person's name, and a breadcrumb is not worth
 *  shipping someone's data for. */
export const describeTarget = (el: Element): string => {
  const tag = el.tagName.toLowerCase()
  const id = el.id ? `#${el.id}` : ''
  // `data-sentori` first: an app that wants a readable trail can say
  // so, and nothing else on the element is safe to assume is not
  // user content.
  const marked = el.getAttribute('data-sentori')
  if (marked) return `${tag}[${marked}]`
  const cls =
    typeof el.className === 'string' && el.className
      ? `.${el.className.trim().split(/\s+/).slice(0, 2).join('.')}`
      : ''
  return `${tag}${id}${cls}`
}

export const installBreadcrumbs = (): void => {
  if (installed || typeof window === 'undefined' || typeof document === 'undefined') return
  installed = true

  onClick = (e: MouseEvent): void => {
    try {
      const t = e.target
      if (t instanceof Element) pushSignal('click', { target: describeTarget(t) })
    } catch (err) {
      reportInternal('breadcrumb-click', err)
    }
  }
  // Capture phase and `passive`: we see the click before a handler can
  // stop it propagating, and the browser knows we will not call
  // `preventDefault`.
  document.addEventListener('click', onClick, { capture: true, passive: true })

  patchHistory()
  window.addEventListener('popstate', () => {
    pushSignal('nav', { to: location.pathname + location.search })
  })
}

/** `pushState` and `replaceState` fire no event, so a single-page app's
 *  navigations are invisible without this. The original runs first and
 *  its result is returned unchanged: if our line throws, the
 *  navigation has already happened. */
const patchHistory = (): void => {
  for (const name of ['pushState', 'replaceState'] as const) {
    const original = history[name]
    if (typeof original !== 'function') continue
    history[name] = function patched(this: History, ...args: Parameters<History['pushState']>) {
      const out = original.apply(this, args)
      try {
        pushSignal('nav', { to: location.pathname + location.search, via: name })
      } catch (err) {
        reportInternal('breadcrumb-nav', err)
      }
      return out
    }
  }
}

export const __resetForTests = (): void => {
  if (onClick && typeof document !== 'undefined') {
    document.removeEventListener('click', onClick, { capture: true })
  }
  onClick = null
  installed = false
}
