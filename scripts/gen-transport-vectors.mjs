#!/usr/bin/env node
// Regenerate the transport-behaviour vectors the native SDKs assert
// against.
//
//   node scripts/gen-transport-vectors.mjs           write
//   node scripts/gen-transport-vectors.mjs --check   fail if stale
//
// Five implementations of one transport now exist: the TypeScript
// kernel that React Native, the browser and the mini program share,
// and a hand-written one each in Swift and Kotlin. They are supposed
// to queue, cap and count losses identically — a `droppedEvents` that
// means different things per platform makes the one number that says
// "you are not seeing everything" unreadable.
//
// Nothing checked that. `identity-vectors` exists because the same
// assumption about hashing turned out to be wrong in two languages;
// this is the same method applied to the behaviour instead of the
// output.
//
// Queue accounting only. The network path needs a server and has its
// own live gates per platform; what belongs here is the arithmetic
// three languages do in three places from the same rules.
//
// Build `@goliapkg/sentori-core` first — this drives the compiled
// kernel, not the source.

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const out = join(root, 'sdk/native/fixtures/transport-vectors.json');
const check = process.argv.includes('--check');

const { createTransport } = await import(join(root, 'sdk/core/lib/index.js'));

/** A transport with nowhere to send: `start()` is never called, so
 *  `flush` returns before touching the network and the queue is the
 *  only thing moving. The numbers below are then about the rules, not
 *  about a server's mood. */
const fresh = () =>
  createTransport({
    config: () => ({ ingestUrl: 'http://127.0.0.1:1', token: 'st_x' }),
    storage: async () => null,
    sdkLabel: 'vectors/0',
    fetch: (async () => {
      throw new Error('the vectors never reach the network');
    }),
  });

const event = (i) => ({
  id: `00000000-0000-7000-8000-${String(i).padStart(12, '0')}`,
  kind: 'error',
  occurredAt: '2026-09-30T00:00:00.000Z',
  platform: 'javascript',
  release: 'vectors@1.0.0',
  environment: 'test',
  payload: {},
});

const CASES = [
  {
    name: 'under the cap nothing is lost',
    ops: [{ op: 'enqueue', n: 9 }],
  },
  {
    name: 'one over the cap drops exactly one',
    ops: [{ op: 'enqueue', n: 501 }],
  },
  {
    // Swift evicts a slice and adds one to `dropped`; Kotlin and the
    // kernel evict in a loop and add one each time. That reads like a
    // divergence and is not: `enqueue` appends a single event, and so
    // does every caller including both offline-queue drains, so the
    // slice is always one element long. Checked rather than assumed,
    // and written down here because the next person to read those
    // three lines will have the same thought.
    //
    // What this case does catch is an implementation that caps the
    // queue and forgets to count — which is the same number reading
    // zero, and indistinguishable from a quiet minute. Removing
    // Kotlin's `dropped += 1` fails it.
    name: 'fifty over the cap drops exactly fifty',
    ops: [{ op: 'enqueue', n: 550 }],
  },
  {
    // Passes and failures land on one row as two deltas. Splitting
    // them would make a pass rate need a join the server does not do.
    name: 'passes and failures aggregate onto one row',
    ops: [
      { op: 'assert', name: 'total.positive', ok: true },
      { op: 'assert', name: 'total.positive', ok: true },
      { op: 'assert', name: 'total.positive', ok: false },
    ],
  },
  {
    // The key is name + release. Two releases reporting the same
    // assert are two rows, or a rollout looks like one number.
    name: 'assert stats key on name and release together',
    ops: [
      { op: 'assert', name: 'a', ok: true, release: 'r1' },
      { op: 'assert', name: 'a', ok: true, release: 'r2' },
      { op: 'assert', name: 'b', ok: true, release: 'r1' },
    ],
  },
  {
    // Sessions have their own cap for their own reason: one lost
    // session moves a published rate, and a flood of events must not
    // be able to take them with it.
    //
    // Kernel only, and said out loud rather than skipped quietly:
    // **neither native transport queues sessions at all**, so an iOS
    // or Android app that is not running React Native contributes no
    // denominator and its crash-free rate is computed from nothing.
    // That is a gap in the product, not in this fixture.
    name: 'sessions cap apart from events',
    applies: ['kernel'],
    ops: [{ op: 'enqueue', n: 600 }, { op: 'session', n: 250 }],
  },
];

const vectors = CASES.map((c) => {
  const t = fresh();
  let counter = 0;
  for (const op of c.ops) {
    if (op.op === 'enqueue') for (let i = 0; i < op.n; i += 1) t.enqueue(event(counter++));
    if (op.op === 'assert') t.countAssert(op.name, op.ok, op.release ?? 'vectors@1.0.0');
    if (op.op === 'session')
      for (let i = 0; i < op.n; i += 1)
        t.queueSession({
          id: `00000000-0000-7000-8000-${String(i).padStart(12, '0')}`,
          status: 'exited',
          release: 'vectors@1.0.0',
          environment: 'test',
          platform: 'javascript',
          startedAt: '2026-09-30T00:00:00.000Z',
          durationMs: 1,
          userId: null,
        });
  }
  return {
    name: c.name,
    // Which implementations must reproduce this. A vector a language
    // cannot run is declared here; a test that skipped one silently
    // would be a gate with a hole in the shape of the thing it was
    // meant to catch.
    applies: c.applies ?? ['kernel', 'swift', 'kotlin'],
    ops: c.ops,
    expect: {
      queued: t.peekQueue().length,
      dropped: t.peekDropped(),
      sessions: t.peekSessions().length,
      assertStats: t
        .peekAssertStats()
        .map((s) => ({ name: s.name, release: s.release, passDelta: s.passDelta, failDelta: s.failDelta ?? 0 }))
        .sort((a, b) => `${a.name}${a.release}`.localeCompare(`${b.name}${b.release}`)),
    },
  };
});

const doc = {
  note:
    'Generated from sdk/core/src/transport.ts — the kernel React Native, the browser ' +
    'and the mini program share. Swift and Kotlin have their own transports and must ' +
    'agree with these numbers. Regenerate with scripts/gen-transport-vectors.mjs; ' +
    'never hand-edit.',
  vectors,
};
const text = `${JSON.stringify(doc, null, 2)}\n`;

if (check) {
  const have = readFileSync(out, 'utf8');
  if (have !== text) {
    console.error('✗ sdk/native/fixtures/transport-vectors.json is stale — the kernel changed.');
    console.error('  Run `node scripts/gen-transport-vectors.mjs`, then make Swift and Kotlin agree.');
    process.exit(1);
  }
  console.log(`✓ ${vectors.length} transport vectors match sdk/core/src/transport.ts`);
} else {
  writeFileSync(out, text);
  console.log(`wrote ${vectors.length} vectors to ${out}`);
}
