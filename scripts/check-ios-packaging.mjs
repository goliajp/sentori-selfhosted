// What an iOS app gets when it depends on this SDK, checked against
// what the sources actually do.
//
//   node scripts/check-ios-packaging.mjs
//
// Apple rejects a submission that calls one of these APIs without a
// declared reason, and the rejection lands on the host app's release,
// not on ours — the one failure mode the client zero-cost rule exists
// to prevent, arriving weeks after the integration and looking like
// the host's own problem.
//
// Two directions. A call with no declaration is the rejection. A
// declaration with no call is a claim about what the SDK does that
// nothing in the SDK does, which is the kind of thing that stops
// being true quietly and stays in the file for years.
//
// Neither the Swift package nor the pod picks the file up implicitly:
// SwiftPM needs it in `resources`, CocoaPods needs it in
// `resource_bundles`, and a pod cannot read the package's copy, so
// there are two files and both are checked.

import { readFileSync, existsSync } from 'node:fs';
import { execFileSync } from 'node:child_process';

const SOURCES = 'sdk/native/ios/Sources/Sentori';
const MANIFEST = `${SOURCES}/PrivacyInfo.xcprivacy`;
const POD_MANIFEST = 'sdk/react-native/ios/core/PrivacyInfo.xcprivacy';
const PACKAGE = 'sdk/native/ios/Package.swift';
const PODSPEC = 'sdk/react-native/SentoriReactNative.podspec';
const NATIVE_PODSPEC = 'sdk/native/ios/Sentori.podspec';

// Symbol → the category Apple files it under. Only the categories
// this SDK could plausibly reach; a new one is added the day a call
// to it is, which is what the "declared but never called" half below
// makes impossible to forget.
const REQUIRED_REASON = [
  ['NSPrivacyAccessedAPICategoryUserDefaults', /\bUserDefaults\b|NSUserDefaults/],
  ['NSPrivacyAccessedAPICategorySystemBootTime', /\bsystemUptime\b|\bmach_absolute_time\b|\bmach_continuous_time\b/],
  [
    'NSPrivacyAccessedAPICategoryFileTimestamp',
    /\bcreationDate\b|\bmodificationDate\b|\battributesOfItem\b|contentModificationDateKey|creationDateKey|\bNSFileCreationDate\b|\bNSFileModificationDate\b|\bgetattrlist\b|\bfstat\b|\blstat\b/,
  ],
  [
    'NSPrivacyAccessedAPICategoryDiskSpace',
    /volumeAvailableCapacity|\bstatfs\b|\bfstatfs\b|NSFileSystemFreeSize|systemFreeSize/,
  ],
  ['NSPrivacyAccessedAPICategoryActiveKeyboards', /activeInputModes|UITextInputMode/],
];

const problems = [];

for (const path of [MANIFEST, POD_MANIFEST]) {
  if (!existsSync(path)) {
    problems.push(`${path} is missing — a binary that reaches a required-reason API without one is rejected`);
  }
}
if (problems.length > 0) {
  for (const p of problems) console.error(`✗ ${p}`);
  process.exit(1);
}

const manifest = readFileSync(MANIFEST, 'utf8');

// Structure before content. `plutil -lint` would be the natural tool
// and is not on the Linux runner this gate has to pass on, so the
// shape is checked here: tags balanced, and the two top-level arrays
// present. A manifest that does not parse is not a manifest, and
// Xcode says so a great deal later than this does.
{
  const tags = [...manifest.matchAll(/<(\/?)(dict|array|plist)\b[^>]*?(\/?)>/g)];
  const stack = [];
  for (const [, closing, tag, selfClosing] of tags) {
    if (selfClosing) continue;
    if (closing) {
      if (stack.pop() !== tag) {
        problems.push(`${MANIFEST}: <${tag}> closes something else — the plist does not parse`);
        break;
      }
    } else {
      stack.push(tag);
    }
  }
  if (stack.length > 0) {
    problems.push(`${MANIFEST}: ${stack.length} unclosed tag(s) — the plist does not parse`);
  }
  for (const key of ['NSPrivacyTracking', 'NSPrivacyCollectedDataTypes', 'NSPrivacyAccessedAPITypes']) {
    if (!manifest.includes(`<key>${key}</key>`)) {
      problems.push(`${MANIFEST} has no ${key} key — Apple treats an absent key as an unanswered question`);
    }
  }
}
if (readFileSync(POD_MANIFEST, 'utf8') !== manifest) {
  problems.push(
    `${POD_MANIFEST} differs from ${MANIFEST} — the pod and the package would ` +
      'declare different things, and only one of them would be wrong in public',
  );
}

// Production sources only: a test target is not shipped, and a
// `UserDefaults` in a test would otherwise force a declaration about
// the product that the product does not earn.
const files = execFileSync('git', ['ls-files', `${SOURCES}/*.swift`], { encoding: 'utf8' })
  .split('\n')
  .filter(Boolean);
if (files.length === 0) {
  console.error(`✗ no tracked Swift sources under ${SOURCES} — this checker would pass on nothing`);
  process.exit(1);
}
const code = files.map((f) => readFileSync(f, 'utf8')).join('\n');

for (const [category, pattern] of REQUIRED_REASON) {
  const used = pattern.test(code);
  const declared = manifest.includes(`<string>${category}</string>`);
  if (used && !declared) {
    const where = files.find((f) => pattern.test(readFileSync(f, 'utf8')));
    problems.push(
      `${where} reaches ${category.replace('NSPrivacyAccessedAPICategory', '')} and ` +
        `${MANIFEST} does not declare it — App Review rejects the host app for this`,
    );
  }
  if (!used && declared) {
    problems.push(
      `${MANIFEST} declares ${category} and nothing under ${SOURCES} calls it — ` +
        'either the call was removed and the claim outlived it, or the pattern here is wrong',
    );
  }
}

// A declared category with an empty reason array is the same as no
// declaration to Apple's checker, and looks like a declaration here.
for (const [category] of REQUIRED_REASON) {
  if (!manifest.includes(`<string>${category}</string>`)) continue;
  const after = manifest.slice(manifest.indexOf(`<string>${category}</string>`));
  const reasons = after.slice(0, after.indexOf('</dict>'));
  if (!/<string>[A-Z0-9]{4}\.\d+<\/string>/.test(reasons)) {
    problems.push(`${category} is declared with no reason code — Apple treats that as undeclared`);
  }
}

// The declaration only ships if the packaging says so.
if (!readFileSync(PACKAGE, 'utf8').includes('PrivacyInfo.xcprivacy')) {
  problems.push(`${PACKAGE} does not put the manifest in the target's resources — SwiftPM would leave it out of the build`);
}
for (const spec of [PODSPEC, NATIVE_PODSPEC]) {
  if (!readFileSync(spec, 'utf8').includes('PrivacyInfo.xcprivacy')) {
    problems.push(
      `${spec} does not ship the manifest — CocoaPods does not pick it up from source_files`,
    );
  }
}

// A pod that names a git repo as its source is installable only if
// it is in that repo. The Swift mirror publishes a subset of this
// directory by an explicit list, so a podspec added here and not
// added there is a pod nobody can install — and nothing would say
// so, because `pod spec lint --quick` never fetches the source.
{
  const mirror = readFileSync('.github/workflows/swift-package-mirror.yml', 'utf8');
  const spec = readFileSync(NATIVE_PODSPEC, 'utf8');
  if (/source\s*=\s*\{\s*git:/.test(spec) && !mirror.includes('Sentori.podspec')) {
    problems.push(
      `${NATIVE_PODSPEC} points at the Swift mirror for its source, and the mirror ` +
        'workflow does not copy it — `pod install` would 404',
    );
  }
}

// Every fixture a mirrored test reads has to reach the mirror.
//
// The tests are copied into the published Swift package; the vectors
// they read live outside it and are copied separately. That copy was
// a list of filenames, so adding a fixture turned the published
// package's own test suite red — in the artifact, after the merge,
// where only a full mirror build could find it.
{
  const testDir = 'sdk/native/ios/Tests';
  const tests = execFileSync('git', ['ls-files', `${testDir}/**/*.swift`], { encoding: 'utf8' })
    .split('\n')
    .filter(Boolean);
  const referenced = new Set();
  for (const file of tests) {
    for (const m of readFileSync(file, 'utf8').matchAll(/"fixtures\/([\w.-]+)"/g)) {
      referenced.add(m[1]);
    }
  }
  const mirror = readFileSync('.github/workflows/swift-package-mirror.yml', 'utf8');
  const copiesTheDirectory = /cp sdk\/native\/fixtures\/\*\.json/.test(mirror);
  // Tracked fixtures only. `live-server.json` is written by
  // `scripts/ios-live-ingest.sh` against a running server and is
  // absent by design; its test skips and says so. A committed fixture
  // is the opposite — its test fails without it, so it must travel.
  const tracked = new Set(
    execFileSync('git', ['ls-files', 'sdk/native/fixtures'], { encoding: 'utf8' })
      .split('\n')
      .filter(Boolean)
      .map((p) => p.slice(p.lastIndexOf('/') + 1)),
  );
  let checked = 0;
  for (const name of referenced) {
    if (!tracked.has(name)) continue;
    checked += 1;
    if (!copiesTheDirectory && !mirror.includes(name)) {
      problems.push(
        `a mirrored test reads fixtures/${name} and the mirror workflow does not copy it — ` +
          "the published package's own tests would fail",
      );
    }
  }
  if (checked === 0) {
    problems.push(
      `no test under ${testDir} reads a committed fixture — this check now reads nothing`,
    );
  }
}

// Three ways in (SwiftPM, the Expo pod, the plain pod) and one
// support statement. A floor that differs between them is a promise
// made in one place and broken in another.
{
  const floors = [PACKAGE, PODSPEC, NATIVE_PODSPEC].map((path) => {
    const src = readFileSync(path, 'utf8');
    const ios = /\.iOS\(\.v(\d+)\)|ios: '(\d+)(?:\.\d+)?'/.exec(src);
    return [path, ios ? (ios[1] ?? ios[2]) : null];
  });
  const stated = floors.filter(([, v]) => v !== null);
  const distinct = new Set(stated.map(([, v]) => v));
  if (stated.length !== 3) {
    problems.push(
      `could not read an iOS floor out of ${floors.filter(([, v]) => v === null).map(([p]) => p).join(', ')}`,
    );
  } else if (distinct.size !== 1) {
    problems.push(
      `the three ways to depend on this SDK state different iOS floors: ` +
        stated.map(([p, v]) => `${p}=${v}`).join(', '),
    );
  }
}

if (problems.length === 0) {
  const declared = REQUIRED_REASON.filter(([c]) => manifest.includes(`<string>${c}</string>`)).length;
  console.log(`✓ privacy manifest declares ${declared} required-reason categor${declared === 1 ? 'y' : 'ies'}, and all three channels ship it`);
  process.exit(0);
}
for (const p of problems) console.error(`✗ ${p}`);
process.exit(1);
