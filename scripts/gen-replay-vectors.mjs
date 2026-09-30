#!/usr/bin/env node
// Regenerate the replay vectors the native SDKs assert against.
//
// The ring decides what a replay is: when a keyframe is due, what
// counts as a change, when a delta is so large a keyframe is cheaper,
// and what falls out of the window. Three implementations of that —
// TypeScript, Swift, Kotlin — that disagree produce a replay which
// plays back a screen the user never saw, and nothing about a wrong
// frame looks wrong. The player will render it without complaint.
//
// So the vectors are generated *by* the source of truth rather than
// written beside it. `sdk/core/src/replay-ring.ts` decides; this file
// records what it decided.
//
//   node scripts/gen-replay-vectors.mjs           write
//   node scripts/gen-replay-vectors.mjs --check   fail if stale
//
// Build `@goliapkg/sentori-core` first — this imports the compiled
// module, not the source.
//
// Compared as parsed JSON, not as bytes: key order differs between a
// JS object literal, Swift's JSONEncoder and Kotlin's JSONObject, and
// the player parses. Asserting byte equality would pin something that
// does not matter and cannot be held.

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = join(root, 'sdk/native/fixtures/replay-vectors.json');
const check = process.argv.includes('--check');

const { ReplayRing } = await import(join(root, 'sdk/core/lib/replay-ring.js'));

const node = (x, y, w, h, extra = {}) => ({ x, y, w, h, ...extra });

// A screen with enough nodes for the keyframe ratio to be in play,
// so the "big transition" branch is reachable rather than theoretical.
const screen = (tint) =>
  Array.from({ length: 12 }, (_, i) => node(0, i * 40, 320, 36, { kind: 'view', color: tint }));

// Each case names the branch it exists for. A case that exercises no
// branch the others miss is a case that will be kept in three
// languages for nothing.
const CASES = [
  {
    name: 'the first frame is a keyframe',
    options: {},
    frames: [{ ts: 1000, width: 320, height: 640, nodes: [node(0, 0, 320, 640, { kind: 'view' })] }],
  },
  {
    name: 'an unchanged frame emits nothing',
    options: {},
    frames: [
      { ts: 1000, width: 320, height: 640, nodes: [node(0, 0, 10, 10)] },
      { ts: 1500, width: 320, height: 640, nodes: [node(0, 0, 10, 10)] },
    ],
  },
  {
    name: 'a small change is a delta',
    options: {},
    frames: [
      { ts: 1000, width: 320, height: 640, nodes: screen('#111') },
      {
        ts: 1500,
        width: 320,
        height: 640,
        nodes: screen('#111').map((n, i) => (i === 0 ? { ...n, color: '#222' } : n)),
      },
    ],
  },
  {
    name: 'a moved node is a removal and an addition, not a change',
    options: {},
    frames: [
      { ts: 1000, width: 320, height: 640, nodes: screen('#111') },
      {
        ts: 1500,
        width: 320,
        height: 640,
        nodes: screen('#111').map((n, i) => (i === 0 ? { ...n, y: 7 } : n)),
      },
    ],
  },
  {
    name: 'two nodes with identical geometry stay two nodes',
    options: {},
    frames: [
      {
        ts: 1000,
        width: 320,
        height: 640,
        nodes: [node(0, 0, 320, 640, { kind: 'view' }), node(0, 0, 320, 640, { kind: 'overlay' })],
      },
      {
        ts: 1500,
        width: 320,
        height: 640,
        nodes: [node(0, 0, 320, 640, { kind: 'view' }), node(0, 0, 320, 640, { kind: 'modal' })],
      },
    ],
  },
  {
    name: 'a near-rewrite becomes a keyframe rather than a huge delta',
    options: {},
    frames: [
      { ts: 1000, width: 320, height: 640, nodes: screen('#111') },
      {
        ts: 1500,
        width: 320,
        height: 640,
        nodes: Array.from({ length: 12 }, (_, i) =>
          node(10, i * 40, 300, 36, { kind: 'view', color: '#999' }),
        ),
      },
    ],
  },
  {
    // Half the screen changing: over the 0.4 threshold, under a
    // careless 0.9. Without a case in this band the threshold could
    // be any number at all and every vector would still pass — the
    // first version of this file had exactly that hole, and moving
    // the Swift constant to 0.9 did not turn the gate red.
    name: 'half the screen changing is already a keyframe',
    options: {},
    frames: [
      { ts: 1000, width: 320, height: 640, nodes: screen('#111') },
      {
        ts: 1500,
        width: 320,
        height: 640,
        nodes: screen('#111').map((n, i) => (i < 6 ? { ...n, color: '#999' } : n)),
      },
    ],
  },
  {
    // Just under the threshold, so the branch is pinned from both
    // sides: a ratio that drifted down would turn this into a
    // keyframe and the case above would not notice.
    name: 'a quarter of the screen changing stays a delta',
    options: {},
    frames: [
      { ts: 1000, width: 320, height: 640, nodes: screen('#111') },
      {
        ts: 1500,
        width: 320,
        height: 640,
        nodes: screen('#111').map((n, i) => (i < 3 ? { ...n, color: '#999' } : n)),
      },
    ],
  },
  {
    // Nine nodes, all of them different: over any ratio, and still a
    // delta, because below the minimum the ratio is noise. This is
    // the only case that pins that floor.
    name: 'a small screen changing entirely is still a delta',
    options: {},
    frames: [
      {
        ts: 1000,
        width: 320,
        height: 640,
        nodes: Array.from({ length: 9 }, (_, i) => node(0, i * 40, 320, 36, { color: '#111' })),
      },
      {
        ts: 1500,
        width: 320,
        height: 640,
        nodes: Array.from({ length: 9 }, (_, i) => node(0, i * 40, 320, 36, { color: '#999' })),
      },
    ],
  },
  {
    name: 'a keyframe comes due on time',
    options: { keyframeMs: 1000 },
    frames: [
      { ts: 1000, width: 320, height: 640, nodes: screen('#111') },
      {
        ts: 1500,
        width: 320,
        height: 640,
        nodes: screen('#111').map((n, i) => (i === 0 ? { ...n, color: '#222' } : n)),
      },
      {
        ts: 2600,
        width: 320,
        height: 640,
        nodes: screen('#111').map((n, i) => (i === 1 ? { ...n, color: '#333' } : n)),
      },
    ],
  },
  {
    name: 'frames older than the window fall out',
    options: { windowMs: 1000 },
    frames: [
      { ts: 1000, width: 320, height: 640, nodes: [node(0, 0, 10, 10, { kind: 'a' })] },
      { ts: 1500, width: 320, height: 640, nodes: [node(0, 0, 10, 10, { kind: 'b' })] },
      { ts: 5000, width: 320, height: 640, nodes: [node(0, 0, 10, 10, { kind: 'c' })] },
    ],
  },
  {
    name: 'the ring is capped however fast the frames arrive',
    options: { maxItems: 3, windowMs: 1_000_000 },
    frames: Array.from({ length: 8 }, (_, i) => ({
      ts: 1000 + i * 100,
      width: 320,
      height: 640,
      nodes: [node(0, 0, 10, 10, { kind: `k${i}` })],
    })),
  },
  {
    name: 'sub-pixel jitter is not a change',
    options: {},
    frames: [
      { ts: 1000, width: 320, height: 640, nodes: [node(0.2, 0.4, 10.6, 10.9, { kind: 'a' })] },
      { ts: 1500, width: 320, height: 640, nodes: [node(0.7, 0.1, 10.2, 10.4, { kind: 'a' })] },
    ],
  },
];

const vectors = CASES.map(({ name, options, frames }) => {
  const ring = new ReplayRing(options);
  const emitted = frames.map((f) => ring.push(f));
  return { name, options, frames, emitted, ring: ring.snapshot() };
});

const body = `${JSON.stringify(vectors, null, 2)}\n`;

if (check) {
  let have = null;
  try {
    have = readFileSync(out, 'utf8');
  } catch {
    console.error(`✗ ${out} does not exist. Run: node scripts/gen-replay-vectors.mjs`);
    process.exit(1);
  }
  if (have !== body) {
    console.error(
      `✗ ${out} is stale — the ring changed and the native ports are still ` +
        'asserting the old behaviour. Run: node scripts/gen-replay-vectors.mjs',
    );
    process.exit(1);
  }
  console.log(`✓ ${vectors.length} replay vectors match sdk/core/src/replay-ring.ts`);
  process.exit(0);
}

mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, body);
console.log(`✓ wrote ${vectors.length} replay vectors to ${out}`);
