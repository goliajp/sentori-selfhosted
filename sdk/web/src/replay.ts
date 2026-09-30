// Wireframe session replay, from the DOM.
//
// The same format the native SDKs produce and the same player reads:
// rectangles with a kind, a colour and — for text — a length rather
// than the text. A second format would mean a second player, and the
// one we have already handles keyframes, deltas and reconstruction.
// That is also why this does not use rrweb: rrweb records the DOM
// itself, which is a different recording, a different player, and a
// far larger payload.
//
// What is captured is shape, never content:
//
//   · every element's box, in page coordinates
//   · what it is — text, image, input, button, or a plain box
//   · its background colour, so the layout is recognisable
//   · for text, the number of characters, so a line is drawn at the
//     right length
//
// The text itself never leaves the page. Neither does an image's src,
// an input's value, or any attribute. A wireframe that carried the
// words would be a screenshot with extra steps, and the whole reason
// this mode exists is to be the one you can leave on.
//
// Off by default anyway: the shape of a page is still information
// about what someone was doing.

import { ReplayRing, logger } from '@goliapkg/sentori-core'
import type { ReplayFrame, ReplayNode } from '@goliapkg/sentori-core'

/** 2 Hz. The walk is the whole cost, and it is linear in the number
 *  of visible elements; twice a second is enough for a player that
 *  cross-fades, and a quarter of the cost of 8 Hz. */
const TICK_INTERVAL_MS = 500

/** Below this the element is a hairline or a spacer, and a wireframe
 *  full of them is unreadable and large. */
const MIN_SIZE_PX = 4

/** A page with more boxes than this is a data grid; past a point the
 *  wireframe stops being a picture and starts being a payload. */
const MAX_NODES = 400

let ring = new ReplayRing()
let timer: null | ReturnType<typeof setInterval> = null
let masked: string[] = []

/**
 * CSS selectors whose subtrees are drawn as a solid block.
 *
 * The same idea as the native registries: an app knows which parts of
 * itself are sensitive and we do not. A masked element keeps its box
 * — the layout stays readable — and contributes nothing about what is
 * inside it, including how much text there is.
 */
export const registerMaskQuery = (...selectors: string[]): void => {
  masked = [...masked, ...selectors.filter((s) => typeof s === 'string' && s.length > 0)]
}

export const startReplay = (): void => {
  if (timer !== null || typeof document === 'undefined') return
  timer = setInterval(() => {
    try {
      const frame = capture()
      if (frame) ring.push(frame)
    } catch (e) {
      // A capture that throws stops the replay rather than throwing
      // twice a second for the life of the page.
      logger.warn(`replay capture failed, stopping: ${String(e)}`)
      stopReplay()
    }
  }, TICK_INTERVAL_MS)
}

export const stopReplay = (): void => {
  if (timer !== null) clearInterval(timer)
  timer = null
}

/** Drain as NDJSON, one entry per line. Empty when nothing is held. */
export const drainReplay = (): string => {
  const entries = ring.drain()
  return entries.length === 0 ? '' : entries.map((e) => JSON.stringify(e)).join('\n')
}

export const capture = (): null | ReplayFrame => {
  if (typeof document === 'undefined' || !document.body) return null

  const maskedEls = new Set<Element>()
  for (const selector of masked) {
    try {
      for (const el of document.querySelectorAll(selector)) maskedEls.add(el)
    } catch {
      // An invalid selector is the host's typo, not a reason to stop
      // capturing the rest.
    }
  }

  const nodes: ReplayNode[] = []
  const walk = (el: Element, insideMask: boolean): void => {
    if (nodes.length >= MAX_NODES) return
    const isMask = insideMask || maskedEls.has(el)
    const rect = el.getBoundingClientRect()
    // Off-screen and zero-area elements are most of a page's DOM and
    // none of its picture.
    const visible =
      rect.width >= MIN_SIZE_PX &&
      rect.height >= MIN_SIZE_PX &&
      rect.bottom > 0 &&
      rect.right > 0 &&
      rect.top < window.innerHeight &&
      rect.left < window.innerWidth

    if (visible) {
      const node = describe(el, rect, isMask)
      if (node) nodes.push(node)
    }
    // Descend even through an invisible parent: a scrolled container
    // is off-screen by its own box and full of children that are not.
    if (!isMask) {
      for (const child of el.children) walk(child, false)
    }
  }
  walk(document.body, false)

  return {
    ts: Date.now(),
    width: window.innerWidth,
    height: window.innerHeight,
    nodes,
  }
}

const describe = (el: Element, rect: DOMRect, isMask: boolean): null | ReplayNode => {
  const style = getComputedStyle(el)
  if (style.visibility === 'hidden' || style.display === 'none' || style.opacity === '0') return null

  const node: ReplayNode = {
    x: Math.round(rect.left),
    y: Math.round(rect.top),
    w: Math.round(rect.width),
    h: Math.round(rect.height),
  }

  if (isMask) {
    node.kind = 'mask'
    return node
  }

  const tag = el.tagName.toLowerCase()
  if (tag === 'img' || tag === 'svg' || tag === 'video' || tag === 'canvas') {
    node.kind = 'image'
  } else if (tag === 'input' || tag === 'textarea' || tag === 'select') {
    node.kind = 'input'
  } else if (tag === 'button' || (tag === 'a' && (el as HTMLAnchorElement).href)) {
    node.kind = 'button'
  }

  // Text, as a length. `textContent` would be the words; what the
  // player needs is how wide to draw the line.
  const own = directText(el)
  if (own > 0) {
    node.kind ??= 'text'
    node.text = 'x'.repeat(Math.min(own, 200))
  }

  const bg = style.backgroundColor
  if (bg && bg !== 'rgba(0, 0, 0, 0)' && bg !== 'transparent') node.color = bg

  // A box with no kind and no colour draws nothing and costs bytes.
  if (!node.kind && !node.color) return null
  return node
}

/** Characters in this element's own text nodes, not its descendants'.
 *  Counting the subtree would draw every ancestor at the length of
 *  the whole page. */
const directText = (el: Element): number => {
  let n = 0
  for (const child of el.childNodes) {
    if (child.nodeType === 3) n += (child.nodeValue ?? '').trim().length
  }
  return n
}

export const __resetReplayForTests = (): void => {
  stopReplay()
  ring = new ReplayRing()
  masked = []
}
