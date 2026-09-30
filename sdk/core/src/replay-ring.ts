// The replay ring: what the last sixty seconds looked like.
//
// This was inside `sdk/react-native/src/replay.ts`, tangled with the
// native module and a `setInterval`, so a native-only app got no
// replay at all — it had the wireframe capture on both platforms and
// nowhere to put the frames. The policy is the part worth having in
// one place: when a keyframe is due, what counts as a change, when a
// delta is so large that a fresh keyframe is cheaper, and what falls
// out of the window.
//
// No IO. The caller owns the timer and the capture; this decides what
// to keep. Swift and Kotlin hold ports of it, checked against vectors
// this file generates, because three implementations of a diff that
// disagree produce a replay that plays back a screen the user never
// saw — and nothing about a wrong frame looks wrong.

/** One rectangle in a captured frame. */
export type ReplayNode = {
  x: number
  y: number
  w: number
  h: number
  kind?: string
  text?: string
  color?: string
}

/** What a platform's wireframe capture hands back. */
export type ReplayFrame = {
  ts: number
  width: number
  height: number
  nodes: ReplayNode[]
}

export type ReplayKeyframe = {
  ts: number
  kind: 'key'
  width: number
  height: number
  nodes: ReplayNode[]
}

export type ReplayDelta = {
  ts: number
  kind: 'delta'
  added: (ReplayNode & { at: number; id: string })[]
  changed: (ReplayNode & { id: string })[]
  removed: { x: number; y: number; w: number; h: number; id: string }[]
}

export type ReplayEntry = ReplayKeyframe | ReplayDelta

export type ReplayRingOptions = {
  /** How often a keyframe is emitted regardless of change. */
  keyframeMs?: number
  /** How long the ring reaches back. */
  windowMs?: number
  /** Hard cap, so a pathological tick rate cannot grow it without end. */
  maxItems?: number
}

const KEYFRAME_INTERVAL_MS = 4_000
const REPLAY_WINDOW_MS = 60_000
const MAX_RING_ITEMS = 1000

/**
 * Past this share of the screen changing, a delta is a near-rewrite
 * and a keyframe reconstructs more cheaply and more reliably.
 */
const DELTA_TO_KEYFRAME_RATIO = 0.4

/** Below this many nodes the ratio is noise — two rectangles moving
 *  on a three-node screen is not a screen transition. */
const KEYFRAME_RATIO_MIN_NODES = 10

/**
 * Identity is geometry plus occurrence index in walk order.
 *
 * Geometry alone is not identity: a page root and the fullscreen
 * overlay covering it share x/y/w/h, and keying on that collapsed
 * them into one node — the overlay inherited the root's z-position
 * and the replay drew the page *through* it. The occurrence suffix
 * gives every stacked layer its own entry and stays stable across
 * ticks, because a stable tree walks in a deterministic order.
 *
 * Rounded to integers first: sub-pixel jitter from a layout engine
 * that occasionally returns floats would otherwise make every node
 * new on every tick, which turns a quiet screen into a stream of
 * full deltas.
 */
export function replayNodeId(node: ReplayNode, occurrence: number): string {
  const round = (v: number) => Math.trunc(v)
  return `${round(node.x)},${round(node.y)},${round(node.w)},${round(node.h)}#${occurrence}`
}

export function indexReplayNodes(nodes: ReplayNode[]): Map<string, { node: ReplayNode; at: number }> {
  const out = new Map<string, { node: ReplayNode; at: number }>()
  const seen = new Map<string, number>()
  nodes.forEach((node, at) => {
    const base = replayNodeId(node, 0).split('#')[0]!
    const occurrence = seen.get(base) ?? 0
    seen.set(base, occurrence + 1)
    out.set(`${base}#${occurrence}`, { node, at })
  })
  return out
}

/**
 * What changed between two indexed frames.
 *
 * Position is identity here, so a node that moved is a removal and an
 * addition rather than a change. Only the things a wireframe draws —
 * kind, colour, text — make an existing node different.
 */
export function computeReplayDelta(
  prev: Map<string, { node: ReplayNode; at: number }>,
  curr: Map<string, { node: ReplayNode; at: number }>,
): Omit<ReplayDelta, 'ts' | 'kind'> {
  const added: ReplayDelta['added'] = []
  const changed: ReplayDelta['changed'] = []
  const removed: ReplayDelta['removed'] = []
  for (const [id, { node, at }] of curr) {
    const before = prev.get(id)
    if (!before) {
      added.push({ ...node, at, id })
      continue
    }
    if (
      (before.node.kind ?? '') !== (node.kind ?? '') ||
      (before.node.color ?? '') !== (node.color ?? '') ||
      (before.node.text ?? '') !== (node.text ?? '')
    ) {
      changed.push({ ...node, id })
    }
  }
  for (const [id, { node }] of prev) {
    if (!curr.has(id)) removed.push({ x: node.x, y: node.y, w: node.w, h: node.h, id })
  }
  return { added, changed, removed }
}

export class ReplayRing {
  private entries: ReplayEntry[] = []
  private lastState: Map<string, { node: ReplayNode; at: number }> | null = null
  private lastKeyframeTs = 0
  private readonly keyframeMs: number
  private readonly windowMs: number
  private readonly maxItems: number

  constructor(options: ReplayRingOptions = {}) {
    this.keyframeMs = options.keyframeMs ?? KEYFRAME_INTERVAL_MS
    this.windowMs = options.windowMs ?? REPLAY_WINDOW_MS
    this.maxItems = options.maxItems ?? MAX_RING_ITEMS
  }

  /**
   * Take a frame. Returns what was appended, or null when the frame
   * was identical to the last one — a heartbeat with nothing in it is
   * dropped, because a minute of a still screen should not cost 120
   * entries that reconstruct to the same picture.
   */
  push(frame: ReplayFrame): ReplayEntry | null {
    const state = indexReplayNodes(frame.nodes)
    const cold = this.lastState === null
    const overdue = frame.ts - this.lastKeyframeTs >= this.keyframeMs

    let entry: ReplayEntry
    if (cold || overdue) {
      entry = this.keyframe(frame)
    } else {
      const delta = computeReplayDelta(this.lastState!, state)
      const touched = delta.added.length + delta.changed.length + delta.removed.length
      if (touched === 0) return null
      if (state.size >= KEYFRAME_RATIO_MIN_NODES && touched >= state.size * DELTA_TO_KEYFRAME_RATIO) {
        entry = this.keyframe(frame)
      } else {
        entry = { ts: frame.ts, kind: 'delta', ...delta }
      }
    }

    this.entries.push(entry)
    this.evict(frame.ts)
    this.lastState = state
    return entry
  }

  /** Everything still inside the window, oldest first. */
  snapshot(): ReplayEntry[] {
    return [...this.entries]
  }

  /**
   * Hand over the ring and start again cold, so the next frame is a
   * keyframe. A drained replay that resumed with a delta would
   * reconstruct against a state the player no longer holds.
   */
  drain(): ReplayEntry[] {
    const out = this.entries
    this.entries = []
    this.lastState = null
    this.lastKeyframeTs = 0
    return out
  }

  private keyframe(frame: ReplayFrame): ReplayKeyframe {
    this.lastKeyframeTs = frame.ts
    return {
      ts: frame.ts,
      kind: 'key',
      width: frame.width,
      height: frame.height,
      nodes: frame.nodes,
    }
  }

  private evict(now: number): void {
    const cutoff = now - this.windowMs
    while (this.entries.length > 0 && this.entries[0]!.ts < cutoff) this.entries.shift()
    while (this.entries.length > this.maxItems) this.entries.shift()
  }
}
