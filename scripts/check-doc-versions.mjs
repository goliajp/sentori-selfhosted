// The install line in the docs installs the version this repo ships.
//
//   node scripts/check-doc-versions.mjs
//
// `docs/sdk-swift.md` said `from: "1.5.0"` and `docs/sdk-kotlin.md`
// said `sentori:1.5.0` while the tree was on 2.1.0 — three minors of
// crash delivery, push and symbolication that a reader following the
// page would not have got, with nothing anywhere saying so. A version
// in prose is a fact about the build, and it rots exactly as quietly
// as any other fact nothing checks.
//
// Only the dependency lines. A version inside an *example* release
// string (`com.example.app@1.5.0+220`) is the reader's app, not ours,
// and pinning it to our number would teach the wrong thing.

import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { execFileSync } from 'node:child_process';

// What a reader can actually install, which is not what the tree is
// about to release.
//
// The first version of this file compared the docs to
// `sdk/native/VERSION`. That is the version being *prepared*, and
// `check-native-version-tag.mjs` requires it to be untagged — so the
// two gates together guaranteed the docs named a version nobody could
// install. They both passed while `from: "2.1.0"` resolved to nothing
// on SwiftPM and Maven Central, whose newest was 2.0.2.
//
// Published means tagged here: `swift/<version>` is written by the
// release that pushes the mirror and the Maven artifact.
const published = execFileSync('git', ['tag', '--list', 'swift/*'], { encoding: 'utf8' })
  .split('\n')
  .map((t) => t.replace('swift/', '').trim())
  .filter((t) => /^\d+\.\d+\.\d+$/.test(t))
  .sort((a, b) => {
    const [x, y] = [a.split('.').map(Number), b.split('.').map(Number)];
    return x[0] - y[0] || x[1] - y[1] || x[2] - y[2];
  });
if (published.length === 0) {
  console.error('✗ no swift/<version> tags — this check cannot tell what is installable');
  process.exit(1);
}
const native = published[published.length - 1];
const nativeMajor = native.split('.')[0];
const rn = JSON.parse(readFileSync('sdk/react-native/package.json', 'utf8')).version;

/**
 * The newest `jp.golia.sentori:sentori` Maven Central will actually
 * serve.
 *
 * Asked of Central rather than inferred from a tag. `swift/2.1.0` was
 * tagged and pushed, this check said the Gradle line could name 2.1.0,
 * and `repo1.maven.org/.../sentori/2.1.0/` answered 404 — the Android
 * publish is a separate manual workflow. A version in a docs snippet
 * is a promise that `gradle build` resolves it, and only the registry
 * can keep that promise.
 *
 * Fails rather than skips when the network is unavailable. A check that
 * goes green because it could not look is the shape of defect this file
 * exists to catch. `SENTORI_SKIP_CENTRAL_CHECK=1` opts out explicitly,
 * for working offline; CI never sets it.
 */
async function newestOnMavenCentral() {
  if (process.env.SENTORI_SKIP_CENTRAL_CHECK === '1') {
    console.warn('! SENTORI_SKIP_CENTRAL_CHECK=1 — the Gradle line is checked against the newest tag instead');
    return native;
  }
  const url =
    'https://repo1.maven.org/maven2/jp/golia/sentori/sentori/maven-metadata.xml';
  let xml;
  try {
    const res = await fetch(url, { signal: AbortSignal.timeout(15000) });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    xml = await res.text();
  } catch (e) {
    console.error(`\u2717 could not ask Maven Central what is published (${e.message}).`);
    console.error('  The Gradle line in the docs is a promise that `gradle build` resolves');
    console.error('  it, and a tag in this repo does not make that true — the Android');
    console.error('  publish is its own manual workflow. Set SENTORI_SKIP_CENTRAL_CHECK=1');
    console.error('  to check against the newest tag instead, offline and knowingly.');
    process.exit(1);
  }
  const versions = [...xml.matchAll(/<version>([\d.]+)<\/version>/g)]
    .map((m) => m[1])
    .filter((v) => /^\d+\.\d+\.\d+$/.test(v))
    .sort((a, b) => {
      const [x, y] = [a.split('.').map(Number), b.split('.').map(Number)];
      return x[0] - y[0] || x[1] - y[1] || x[2] - y[2];
    });
  if (versions.length === 0) {
    console.error('\u2717 Maven Central listed no versions of jp.golia.sentori:sentori');
    process.exit(1);
  }
  return versions[versions.length - 1];
}

const CASES = [
  {
    file: 'docs/sdk-swift.md',
    pattern: /sentori-swift",\s*from:\s*"(\d+)\.\d+\.\d+"/,
    want: nativeMajor,
    what: 'the Swift Package Manager line',
    source: 'the newest tag',
    note:
      '`from:` is a floor that resolves to the newest release in that major, ' +
      'so it names the major rather than a version that would go stale on every release',
  },
  {
    file: 'docs/sdk-kotlin.md',
    pattern: /jp\.golia\.sentori:sentori:([\d.]+)/,
    // Not `native`. A tag here means the Swift mirror went out; the
    // Android artifact is a separate, manual publish to Maven Central,
    // and for a while it was over the org's monthly quota. So the tag
    // and the Gradle coordinate are different facts, and this is the
    // one case where only the registry knows the answer.
    want: await newestOnMavenCentral(),
    what: 'the Gradle line',
    source: 'Maven Central',
  },
];

const problems = [];
for (const { file, pattern, want, what, source } of CASES) {
  const found = pattern.exec(readFileSync(file, 'utf8'));
  if (!found) {
    problems.push(`${file}: could not find ${what} — it moved, and this check now reads nothing`);
    continue;
  }
  if (found[1] !== want) {
    problems.push(
      `${file}: ${what} names ${found[1]}, and ${source ?? 'this repo'} has ${want} — ` +
        'a reader following the page cannot resolve it',
    );
  }
}

// The React Native package advertises itself in its own README, which
// npm renders on the package page.
{
  const readme = readFileSync('sdk/react-native/README.md', 'utf8');
  const stale = [...readme.matchAll(/@goliapkg\/sentori-react-native@([\d.]+)/g)]
    .map((m) => m[1])
    .filter((v) => v !== rn);
  for (const v of stale) {
    problems.push(`sdk/react-native/README.md names ${v}; the package is ${rn}`);
  }
}

// Every npm package an install line names has to be on npm.
//
// This file checked SwiftPM and Maven Central and never once asked the
// registry the other four SDKs come from. `docs/getting-started/web.md`
// said `bun add @goliapkg/sentori-web` and
// `docs/getting-started/weapp.md` said
// `npm install @goliapkg/sentori-weapp` for two packages that had never
// been published — v4's headline is five SDKs on one wire and two of
// them 404'd, past the gate whose entire job is that install lines
// install something.
//
// Same discipline as the Maven check: a registry that cannot be reached
// fails rather than skips.
const npmNamed = new Set();
for (const file of [
  ...readdirSync('docs/getting-started').map((f) => `docs/getting-started/${f}`),
  ...readdirSync('sdk')
    .map((p) => `sdk/${p}/README.md`)
    .filter((f) => existsSync(f)),
  'docs/getting-started.md',
]) {
  if (!file.endsWith('.md')) continue;
  for (const m of readFileSync(file, 'utf8').matchAll(
    /(?:npm install|npm i|bun add|yarn add|pnpm add)\s+(@goliapkg\/[\w-]+)/g,
  )) {
    npmNamed.add(m[1]);
  }
}
if (npmNamed.size < 2) {
  problems.push(
    `found ${npmNamed.size} npm install line(s) in the docs — this check is reading ` +
      'nothing and must not pass',
  );
}
for (const pkg of [...npmNamed].sort()) {
  let res;
  try {
    res = await fetch(`https://registry.npmjs.org/${pkg.replace('/', '%2f')}`, {
      signal: AbortSignal.timeout(15000),
    });
  } catch (e) {
    console.error(`✗ could not ask npm about ${pkg} (${e.message}).`);
    console.error('  An install line is a promise that the install works, and only the');
    console.error('  registry can keep it. This check fails rather than guessing.');
    process.exit(1);
  }
  if (res.status === 404) {
    problems.push(
      `${pkg} is named in an install line and is not on npm — a reader following ` +
        'the page gets E404',
    );
  } else if (!res.ok) {
    console.error(`✗ npm answered HTTP ${res.status} for ${pkg}`);
    process.exit(1);
  }
}

if (problems.length === 0) {
  console.log(
    `✓ every advertised version is one a reader can install ` +
      `(swift ${native}, maven ${CASES[1].want}, rn ${rn}, ${npmNamed.size} npm packages)`,
  );
  process.exit(0);
}
for (const p of problems) console.error(`✗ ${p}`);
process.exit(1);
