// An issue's history records decisions, not requests.
//
//   SENTORI_BASE=… SENTORI_JAR=… SENTORI_ISSUE=… node scripts/check-issue-activity.mjs
//
// Resolving an already-resolved issue used to append a row every time.
// Thirty concurrent status calls left thirty rows for what an operator
// experienced as a handful of decisions, and the panel that exists to
// show an issue's history became the panel that hides it.
//
// The rule: a status write that changes nothing records nothing, and a
// write that changes something records once — including under
// concurrency, where the database's predicate is what decides.
import { execFile, execFileSync } from 'node:child_process';

const BASE = process.env.SENTORI_BASE;
const JAR = process.env.SENTORI_JAR;
const ISSUE = process.env.SENTORI_ISSUE;
if (!BASE || !JAR || !ISSUE) {
  console.error('set SENTORI_BASE, SENTORI_JAR and SENTORI_ISSUE');
  process.exit(2);
}

const curl = (args) =>
  execFileSync('curl', ['-s', '-b', JAR, ...args], { encoding: 'utf8' });

const post = (path, body = '{}') =>
  curl(['-X', 'POST', `${BASE}/admin/api/issues/${ISSUE}/${path}`,
    '-H', 'content-type: application/json', '-d', body]);

const activityCount = () => {
  const raw = curl([`${BASE}/admin/api/issues/${ISSUE}`]);
  try {
    return (JSON.parse(raw).activity ?? []).length;
  } catch {
    console.error(`✗ could not read the issue: ${raw.slice(0, 160)}`);
    process.exit(2);
  }
};

// Establish a known state, then measure from there.
post('reopen');
const base = activityCount();

post('resolve', '{"release":"app@1.0.0+1"}');
const afterFirst = activityCount();
if (afterFirst !== base + 1) {
  console.error(`✗ resolving an open issue recorded ${afterFirst - base} rows, expected 1`);
  process.exit(1);
}

// The same resolve again, twice. Nothing changes, so nothing is filed.
post('resolve', '{"release":"app@1.0.0+1"}');
post('resolve', '{"release":"app@1.0.0+1"}');
const afterRepeat = activityCount();
if (afterRepeat !== afterFirst) {
  console.error(
    `✗ two repeated resolves added ${afterRepeat - afterFirst} row(s). ` +
      `A retry is not a decision.`,
  );
  process.exit(1);
}

// A different release IS a change and must be recorded.
post('resolve', '{"release":"app@2.0.0+9"}');
const afterRelease = activityCount();
if (afterRelease !== afterRepeat + 1) {
  console.error(
    `✗ resolving into a different release recorded ${afterRelease - afterRepeat} ` +
      `rows, expected 1 — that is a real change and must not be swallowed.`,
  );
  process.exit(1);
}

// Concurrency: ten simultaneous ignores, one transition.
//
// Through the same cookie jar the rest of this file uses. The first
// version hand-assembled a Cookie header for `fetch`, got it wrong,
// and every request 401'd — which produced zero new rows and read
// exactly like the assertion passing. The statuses are checked for
// that reason: zero rows is only meaningful if the writes happened.
const before = activityCount();
const statuses = await Promise.all(
  Array.from({ length: 10 }, () =>
    new Promise((resolve) => {
      execFile(
        'curl',
        ['-s', '-o', '/dev/null', '-w', '%{http_code}', '-b', JAR, '-X', 'POST',
          `${BASE}/admin/api/issues/${ISSUE}/ignore`,
          '-H', 'content-type: application/json', '-d', '{}'],
        (_e, out) => resolve((out ?? '').trim()),
      );
    })),
);
const ok = statuses.filter((c) => c === '200').length;
if (ok !== statuses.length) {
  console.error(
    `✗ only ${ok} of ${statuses.length} concurrent ignores were accepted ` +
      `(${[...new Set(statuses)].join(', ')}). This harness proved nothing.`,
  );
  process.exit(2);
}
const after = activityCount();
if (after - before !== 1) {
  console.error(
    `✗ ten concurrent ignores recorded ${after - before} rows, expected 1. ` +
      `The predicate on the UPDATE is what makes this exactly once.`,
  );
  process.exit(1);
}

console.log('✓ status writes record a decision, not a request (4 assertions)');
