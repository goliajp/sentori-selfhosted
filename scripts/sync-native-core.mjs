#!/usr/bin/env node
// The native core has one editable home; the React Native package
// carries a mirror of it.
//
// `sdk/native/ios` is a Swift Package apps without React Native can
// link, and it is where these sources are edited. The RN package still
// has to ship them inside its npm tarball — CocoaPods resolves
// `source_files` relative to the podspec, so a pod cannot reach up out
// of the package directory, and npm cannot follow a symlink into a
// tarball.
//
// So: copy, and gate the copy. A mirror CI proves byte-identical is
// not the kind of duplication that drifts; a hand-maintained second
// implementation is, and that is exactly what insight declined to
// write for the HTTP contract (2026-08-11).
//
//   node scripts/sync-native-core.mjs          write the mirror
//   node scripts/sync-native-core.mjs --check  fail if it is stale
//
// The mirror is committed rather than generated at install time: a
// fresh clone runs `expo prebuild` without any Sentori build step, and
// a pod whose sources appear only after someone remembers a command is
// a pod that fails for whoever did not.

import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const check = process.argv.includes('--check');

const MIRRORS = [
  {
    from: 'sdk/native/ios/Sources/Sentori',
    to: 'sdk/react-native/ios/core',
    ext: '.swift',
    // The standalone SDK's own transport, config, scope, signal ring
    // and identity hashing. React Native already has all five in
    // JavaScript and calls those, so mirroring these would compile a
    // second unused copy into every RN app's binary — a cost the host
    // did not ask for, which is the whole footprint half of the client
    // zero-cost rule. When the bridge starts delegating to them
    // (v1.6 S4), the exclusion goes.
    exclude: [
      'Sentori.swift',
      'SentoriConfig.swift',
      'SentoriIdentity.swift',
      'SentoriDevice.swift',
      'SentoriPendingCrash.swift',
      'SentoriPush.swift',
      'SentoriScope.swift',
      'SentoriSignalRing.swift',
      'SentoriTransport.swift',
    ],
  },
  // Android mirrors into its own source root rather than beside
  // `SentoriModule.kt`: the sync owns its target directory outright
  // (it deletes before writing, so a file removed upstream cannot
  // survive), and the bridge must not live somewhere that gets wiped.
  // Kotlin package names are declared in the file, not derived from
  // the path, so `com.sentori` still resolves from a second root.
  {
    from: 'sdk/native/android/src/main/java/com/sentori',
    to: 'sdk/react-native/android/src/main/java-core/com/sentori',
    ext: '.kt',
  },
  // The tests move with the code they cover. Without this the two
  // Robolectric suites would leave the RN module's test source set and
  // `android-unit` would go on passing while running strictly less —
  // which is how they spent their first two years, unable to compile
  // and unnoticed.
  {
    from: 'sdk/native/android/src/test/java/com/sentori',
    to: 'sdk/react-native/android/src/test/java-core/com/sentori',
    ext: '.kt',
  },
];

const BANNER = (rel) =>
  `// GENERATED MIRROR — do not edit.\n` +
  `// Source of truth: ${rel}\n` +
  `// Run \`node scripts/sync-native-core.mjs\` after editing it.\n`;

let stale = [];
let written = 0;
let compared = 0;

for (const m of MIRRORS) {
  const src = join(root, m.from);
  const dst = join(root, m.to);
  if (!existsSync(src)) {
    console.error(`✗ ${m.from} does not exist — this list is stale.`);
    process.exit(1);
  }
  const names = readdirSync(src)
    .filter((f) => f.endsWith(m.ext))
    .filter((f) => !(m.exclude ?? []).includes(f));
  if (names.length === 0) {
    console.error(`✗ no ${m.ext} files under ${m.from} — this script would mirror nothing.`);
    process.exit(1);
  }

  if (!check) {
    // Remove first, so a file deleted upstream does not survive in the
    // mirror and keep compiling into the pod.
    rmSync(dst, { recursive: true, force: true });
    mkdirSync(dst, { recursive: true });
  }

  for (const name of names) {
    const want = BANNER(`${m.from}/${name}`) + readFileSync(join(src, name), 'utf8');
    const out = join(dst, name);
    if (check) {
      compared += 1;
      const have = existsSync(out) ? readFileSync(out, 'utf8') : null;
      if (have !== want) stale.push(`${m.to}/${name}`);
    } else {
      writeFileSync(out, want);
      written += 1;
    }
  }

  if (check && existsSync(dst)) {
    for (const extra of readdirSync(dst).filter((f) => f.endsWith(m.ext))) {
      compared += 1;
      if (!names.includes(extra)) stale.push(`${m.to}/${extra} (no longer in ${m.from})`);
    }
  }
}

// The mirror moves sources. It does not move what they need to
// compile — and the tests move too, so a test dependency that exists
// only in `sdk/native` leaves the mirror compiling and its tests not.
// That is exactly what happened when a test started driving a real
// `RemoteMessage`: `sdk/native` went green and CI went red on a
// module whose only difference was one line of Gradle.
const GRADLE_PAIR = [
  'sdk/native/android/build.gradle',
  'sdk/react-native/android/build.gradle',
];
function testDeps(path) {
  return new Set(
    [...readFileSync(join(root, path), 'utf8').matchAll(/testImplementation\s+['"]([^'"]+)['"]/g)]
      // The Kotlin one is interpolated on one side and literal on the
      // other; the version is not what this is about.
      .map((m) => m[1].split(':').slice(0, 2).join(':')),
  );
}
const [homeDeps, mirrorDeps] = GRADLE_PAIR.map(testDeps);
const missing = [...homeDeps].filter((d) => !mirrorDeps.has(d));
if (homeDeps.size === 0) {
  console.error(`✗ read no testImplementation lines out of ${GRADLE_PAIR[0]}`);
  process.exit(1);
}
if (missing.length > 0) {
  for (const d of missing) {
    console.error(
      `✗ ${GRADLE_PAIR[0]} tests against '${d}' and ${GRADLE_PAIR[1]} does not — ` +
        'the mirrored tests will not compile there',
    );
  }
  process.exit(1);
}

if (!check) {
  console.log(`✓ mirrored ${written} native source(s) into the React Native package`);
  process.exit(0);
}
if (compared === 0) {
  console.error('✗ nothing was compared — this checker read nothing.');
  process.exit(1);
}
if (stale.length === 0) {
  console.log(`✓ ${compared} mirrored native source(s) match their home in sdk/native`);
  process.exit(0);
}
for (const s of stale) console.error(`✗ out of sync: ${s}`);
console.error('\nRun `node scripts/sync-native-core.mjs`. Edit sdk/native, never the mirror.');
process.exit(1);
