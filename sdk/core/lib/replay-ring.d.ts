/** One rectangle in a captured frame. */
export type ReplayNode = {
    x: number;
    y: number;
    w: number;
    h: number;
    kind?: string;
    text?: string;
    color?: string;
};
/** What a platform's wireframe capture hands back. */
export type ReplayFrame = {
    ts: number;
    width: number;
    height: number;
    nodes: ReplayNode[];
};
export type ReplayKeyframe = {
    ts: number;
    kind: 'key';
    width: number;
    height: number;
    nodes: ReplayNode[];
};
export type ReplayDelta = {
    ts: number;
    kind: 'delta';
    added: (ReplayNode & {
        at: number;
        id: string;
    })[];
    changed: (ReplayNode & {
        id: string;
    })[];
    removed: {
        x: number;
        y: number;
        w: number;
        h: number;
        id: string;
    }[];
};
export type ReplayEntry = ReplayKeyframe | ReplayDelta;
export type ReplayRingOptions = {
    /** How often a keyframe is emitted regardless of change. */
    keyframeMs?: number;
    /** How long the ring reaches back. */
    windowMs?: number;
    /** Hard cap, so a pathological tick rate cannot grow it without end. */
    maxItems?: number;
};
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
export declare function replayNodeId(node: ReplayNode, occurrence: number): string;
export declare function indexReplayNodes(nodes: ReplayNode[]): Map<string, {
    node: ReplayNode;
    at: number;
}>;
/**
 * What changed between two indexed frames.
 *
 * Position is identity here, so a node that moved is a removal and an
 * addition rather than a change. Only the things a wireframe draws —
 * kind, colour, text — make an existing node different.
 */
export declare function computeReplayDelta(prev: Map<string, {
    node: ReplayNode;
    at: number;
}>, curr: Map<string, {
    node: ReplayNode;
    at: number;
}>): Omit<ReplayDelta, 'ts' | 'kind'>;
export declare class ReplayRing {
    private entries;
    private lastState;
    private lastKeyframeTs;
    private readonly keyframeMs;
    private readonly windowMs;
    private readonly maxItems;
    constructor(options?: ReplayRingOptions);
    /**
     * Take a frame. Returns what was appended, or null when the frame
     * was identical to the last one — a heartbeat with nothing in it is
     * dropped, because a minute of a still screen should not cost 120
     * entries that reconstruct to the same picture.
     */
    push(frame: ReplayFrame): ReplayEntry | null;
    /** Everything still inside the window, oldest first. */
    snapshot(): ReplayEntry[];
    /**
     * Hand over the ring and start again cold, so the next frame is a
     * keyframe. A drained replay that resumed with a delta would
     * reconstruct against a state the player no longer holds.
     */
    drain(): ReplayEntry[];
    private keyframe;
    private evict;
}
//# sourceMappingURL=replay-ring.d.ts.map