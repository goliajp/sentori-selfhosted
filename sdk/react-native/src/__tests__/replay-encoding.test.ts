import { afterEach, describe, expect, test } from 'bun:test';

import {
  __feedTickForTests,
  __resetReplayForTests,
  computeDelta,
  drainReplay,
} from '../replay';

// Unit coverage for the rc.9 v2 replay encoder.
//
// The encoder lives in replay.ts; native module is mocked away via
// __feedTickForTests so we drive the captureTick body with hand-built
// snapshots and inspect the resulting NDJSON shape.
//
// See docs/replay-encoding-v2.md for the wire schema.

afterEach(() => {
  __resetReplayForTests();
});

type Node = {
  x: number;
  y: number;
  w: number;
  h: number;
  kind?: string;
  text?: string;
  color?: string;
};

function frame(ts: number, nodes: Node[]) {
  return JSON.stringify({ ts, width: 1080, height: 2340, nodes });
}

function nodeRect(x: number, y: number, w: number, h: number, color = '#FF0000FF'): Node {
  return { x, y, w, h, kind: 'rect', color };
}

function nodeText(x: number, y: number, w: number, h: number, text: string, color = '#FFFFFFFF'): Node {
  return { x, y, w, h, kind: 'text', text, color };
}

function readDrainedLines(): unknown[] {
  const drained = drainReplay();
  if (drained.length === 0) return [];
  return drained.split('\n').map((l) => JSON.parse(l) as unknown);
}

describe('rc.9 encoder — keyframe vs delta', () => {
  test('first tick after start always emits a keyframe', () => {
    __feedTickForTests(frame(1_000, [nodeRect(0, 0, 1080, 2340)]));
    const lines = readDrainedLines();
    expect(lines.length).toBe(1);
    expect((lines[0] as { kind: string }).kind).toBe('key');
    expect((lines[0] as { width: number }).width).toBe(1080);
    expect((lines[0] as { nodes: Node[] }).nodes.length).toBe(1);
  });

  test('static UI ticks emit no line (no-op heartbeat dropped)', () => {
    const nodes = [nodeRect(0, 0, 1080, 2340, '#000000FF')];
    __feedTickForTests(frame(1_000, nodes));
    __feedTickForTests(frame(1_250, nodes));
    __feedTickForTests(frame(1_500, nodes));
    __feedTickForTests(frame(1_750, nodes));
    const lines = readDrainedLines();
    // 1 keyframe + 0 deltas (all subsequent ticks are no-op)
    expect(lines.length).toBe(1);
    expect((lines[0] as { kind: string }).kind).toBe('key');
  });

  test('a single node change emits exactly one delta with the changed node', () => {
    const baseline = [
      nodeRect(0, 0, 1080, 2340, '#0E0E10FF'),
      nodeText(60, 192, 960, 112, 'before'),
    ];
    const after = [
      nodeRect(0, 0, 1080, 2340, '#0E0E10FF'),
      nodeText(60, 192, 960, 112, 'after'),
    ];
    __feedTickForTests(frame(1_000, baseline));
    __feedTickForTests(frame(1_250, after));
    const lines = readDrainedLines();
    expect(lines.length).toBe(2);
    expect((lines[0] as { kind: string }).kind).toBe('key');
    expect((lines[1] as { kind: string }).kind).toBe('delta');
    const d = lines[1] as { added: Node[]; changed: Node[]; removed: Node[] };
    expect(d.added.length).toBe(0);
    expect(d.removed.length).toBe(0);
    expect(d.changed.length).toBe(1);
    expect((d.changed[0] as Node).text).toBe('after');
  });

  test('added + removed nodes both appear in the delta', () => {
    const baseline = [nodeRect(0, 0, 1080, 100), nodeRect(0, 200, 1080, 100)];
    const after = [nodeRect(0, 0, 1080, 100), nodeRect(0, 400, 1080, 100)];
    __feedTickForTests(frame(1_000, baseline));
    __feedTickForTests(frame(1_250, after));
    const lines = readDrainedLines();
    expect(lines.length).toBe(2);
    const d = lines[1] as { added: Node[]; changed: Node[]; removed: Node[] };
    expect(d.added.length).toBe(1);
    expect((d.added[0] as Node).y).toBe(400);
    expect(d.removed.length).toBe(1);
    expect((d.removed[0] as Node).y).toBe(200);
  });

  test('keyframe overdue ⇒ next emit is a keyframe even when delta is small', () => {
    const baseline = [nodeRect(0, 0, 1080, 2340), nodeText(60, 192, 960, 112, 'a')];
    // After 4 s (default keyframeMs), the next change triggers a fresh keyframe.
    __feedTickForTests(frame(1_000, baseline));
    const slightlyChanged = [
      nodeRect(0, 0, 1080, 2340),
      nodeText(60, 192, 960, 112, 'a-changed'),
    ];
    __feedTickForTests(frame(1_000 + 4_001, slightlyChanged));
    const lines = readDrainedLines();
    expect(lines.length).toBe(2);
    expect((lines[0] as { kind: string }).kind).toBe('key');
    expect((lines[1] as { kind: string }).kind).toBe('key'); // overdue → key
  });

  test('big screen transition (>40% nodes change) prefers a keyframe over a huge delta', () => {
    const baseline = Array.from({ length: 10 }, (_, i) => nodeRect(0, i * 100, 1080, 90));
    const after = Array.from({ length: 10 }, (_, i) => nodeRect(0, i * 100 + 500, 1080, 90));
    __feedTickForTests(frame(1_000, baseline));
    __feedTickForTests(frame(1_250, after));
    const lines = readDrainedLines();
    expect(lines.length).toBe(2);
    expect((lines[0] as { kind: string }).kind).toBe('key');
    // Delta would be 10 added + 10 removed = 20 changes, > 10 * 0.4 threshold,
    // so encoder should fall back to keyframe instead.
    expect((lines[1] as { kind: string }).kind).toBe('key');
  });

  test('reconstructing keyframe + applied deltas recovers identical final state', () => {
    // Drive 6 ticks: keyframe + 5 small deltas, then assert reconstruction equals the last input state.
    const finalNodes: Node[] = [];
    const baseline = [nodeRect(0, 0, 1080, 2340, '#0E0E10FF')];
    __feedTickForTests(frame(1_000, baseline));
    let state: Node[] = [...baseline];
    for (let i = 0; i < 5; i++) {
      // Add one row per tick.
      const row = nodeText(60, 200 + i * 100, 960, 80, `row ${i}`);
      state = [...state, row];
      __feedTickForTests(frame(1_000 + (i + 1) * 250, state));
    }
    finalNodes.push(...state);

    const lines = readDrainedLines();
    expect(lines.length).toBe(6);
    // Reconstruct: start from keyframe, apply each delta in order.
    let reconstructed = new Map<string, Node>();
    for (const line of lines) {
      const l = line as { kind: string; nodes?: Node[]; added?: Node[]; changed?: Node[]; removed?: Pick<Node, 'x' | 'y' | 'w' | 'h'>[] };
      if (l.kind === 'key') {
        reconstructed = new Map((l.nodes ?? []).map((n) => [`${n.x | 0},${n.y | 0},${n.w | 0},${n.h | 0}`, n]));
      } else {
        for (const r of l.removed ?? []) {
          reconstructed.delete(`${r.x | 0},${r.y | 0},${r.w | 0},${r.h | 0}`);
        }
        for (const a of l.added ?? []) {
          reconstructed.set(`${a.x | 0},${a.y | 0},${a.w | 0},${a.h | 0}`, a);
        }
        for (const c of l.changed ?? []) {
          reconstructed.set(`${c.x | 0},${c.y | 0},${c.w | 0},${c.h | 0}`, c);
        }
      }
    }
    expect(reconstructed.size).toBe(finalNodes.length);
    for (const n of finalNodes) {
      const got = reconstructed.get(`${n.x | 0},${n.y | 0},${n.w | 0},${n.h | 0}`);
      expect(got).toBeDefined();
      expect(got!.kind).toBe(n.kind);
      expect(got!.text).toBe(n.text);
      expect(got!.color).toBe(n.color);
    }
  });

  test('byte budget — 60 s × 4 Hz dense UI stays below the rc.8 baseline', () => {
    // Simulate 60 s at 4 Hz = 240 ticks. Dense UI: 100 nodes, 2 nodes
    // change per tick on average. This mirrors the typical
    // mostly-static-with-small-animations app shape.
    const baseNodes = Array.from({ length: 100 }, (_, i) => nodeRect(0, i * 24, 1080, 20));
    let totalBytes = 0;
    const tickCount = 240;
    for (let t = 0; t < tickCount; t++) {
      // Every 4th tick mutates two nodes.
      const nodes = baseNodes.map((n, i) => {
        if (t % 4 === 0 && (i === t % 100 || i === (t + 50) % 100)) {
          return { ...n, color: t & 1 ? '#FF0000FF' : '#00FF00FF' };
        }
        return n;
      });
      __feedTickForTests(frame(1_000 + t * 250, nodes));
    }
    const drained = drainReplay();
    totalBytes = drained.length;

    // rc.8 baseline for the same window: 60 × 100 nodes × ~50 bytes/node ≈ 300 KB.
    // rc.9 target: well under 200 KB.
    expect(totalBytes).toBeLessThan(200_000);
    // Sanity: not zero, and dominated by keyframes.
    expect(totalBytes).toBeGreaterThan(10_000);
  });
});

describe('computeDelta', () => {
  function asMap(nodes: Node[]): Map<string, Node> {
    return new Map(nodes.map((n) => [`${n.x | 0},${n.y | 0},${n.w | 0},${n.h | 0}`, n]));
  }

  test('empty-vs-empty produces all-empty delta', () => {
    const d = computeDelta(new Map(), new Map());
    expect(d.added.length).toBe(0);
    expect(d.changed.length).toBe(0);
    expect(d.removed.length).toBe(0);
  });

  test('identical state produces no delta', () => {
    const m = asMap([nodeRect(0, 0, 10, 10), nodeRect(0, 10, 10, 10)]);
    const d = computeDelta(m, m);
    expect(d.added.length).toBe(0);
    expect(d.changed.length).toBe(0);
    expect(d.removed.length).toBe(0);
  });

  test('integer-rounds fingerprints so sub-pixel jitter does not register', () => {
    const prev = asMap([{ x: 0.1, y: 0.2, w: 10.3, h: 10.4, kind: 'rect' }]);
    const curr = asMap([{ x: 0.4, y: 0.3, w: 10.2, h: 10.1, kind: 'rect' }]);
    // Fingerprint of both is '0,0,10,10' → matched, no diff fields differ.
    const d = computeDelta(prev, curr);
    expect(d.added.length).toBe(0);
    expect(d.changed.length).toBe(0);
    expect(d.removed.length).toBe(0);
  });
});
