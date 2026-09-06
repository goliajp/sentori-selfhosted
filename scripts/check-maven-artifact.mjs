#!/usr/bin/env node
// The Android artifact is complete enough for Maven Central.
//
// Central validates the POM at upload and rejects the whole bundle if
// anything required is missing — a slow way to learn that a `<scm>`
// block was dropped, and slower still because a rejected release has
// to be re-staged rather than fixed in place.
//
// Run after `publishReleasePublicationToBuildDirRepository`, which
// stages the exact files that would be uploaded.
//
//   node scripts/check-maven-artifact.mjs
//
// Signatures are not checked here: they only exist when SIGNING_KEY is
// set, which is a release, and this has to pass on every push.

import { existsSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const staging = join(root, 'sdk/native/android/build/staging-repo');

if (!existsSync(staging)) {
  console.error(
    '✗ no staged artifact. Run:\n' +
      '    (cd sdk/native/android && ./gradlew publishReleasePublicationToBuildDirRepository)',
  );
  process.exit(1);
}

function walk(dir, out = []) {
  for (const e of readdirSync(dir)) {
    const p = join(dir, e);
    if (statSync(p).isDirectory()) walk(p, out);
    else out.push(p);
  }
  return out;
}

const files = walk(staging).map((f) => f.replace(`${staging}/`, ''));
const problems = [];

// Central requires the artifact, its sources and its javadoc. A
// consumer with no sources jar gets no step-through debugging into the
// SDK, which for a crash reporter is the wrong way round.
for (const suffix of ['.aar', '-sources.jar', '-javadoc.jar', '.pom']) {
  if (!files.some((f) => f.endsWith(suffix))) {
    problems.push(`no ${suffix} in the staged artifact`);
  }
}

const pomPath = walk(staging).find((f) => f.endsWith('.pom'));
if (!pomPath) {
  console.error('✗ no POM staged — nothing else can be checked.');
  process.exit(1);
}
const pom = readFileSync(pomPath, 'utf8');

// Exactly what Central's validation looks for.
const required = {
  '<name>': 'a human name',
  '<description>': 'a description',
  '<url>': 'a project url',
  '<licenses>': 'at least one license',
  '<developers>': 'a developer',
  '<scm>': 'source control coordinates',
  '<connection>': 'an scm connection',
};
for (const [tag, what] of Object.entries(required)) {
  if (!pom.includes(tag)) problems.push(`POM has no ${tag} — Central wants ${what}`);
}

// A SNAPSHOT cannot be released, and a version that is still the
// default placeholder means the release workflow did not pass one in.
const version = /<version>([^<]+)<\/version>/.exec(pom)?.[1] ?? '';
if (version.endsWith('-SNAPSHOT')) problems.push(`version ${version} is a snapshot`);
if (!/^\d+\.\d+\.\d+/.test(version)) problems.push(`version ${version} is not semver`);

// The staged artifact is whatever was last built, and nothing expires
// it. This gate reported `1.0.0` green while the tree said 1.2.0 — it
// was validating a POM from a release two versions old, which is a
// gate answering a question about a file nobody is going to publish.
const declared = readFileSync(join(root, 'sdk/native/VERSION'), 'utf8').trim();
if (version && !version.startsWith(declared)) {
  problems.push(
    `the staged artifact is ${version} but sdk/native/VERSION says ${declared} — ` +
      'this is stale output from an earlier build. Re-stage with\n' +
      '    rm -rf sdk/native/android/build/staging-repo\n' +
      '    (cd sdk/native/android && ./gradlew publishReleasePublicationToBuildDirRepository)\n' +
      // Publishing without clearing leaves both versions in the tree,
      // and this check reads whichever POM it walks into first — so
      // following the old wording produced the same message again.
      // The release script has always cleared first; the advice here
      // had not.\n' +
      '    (the release script already does both)',
  );
}

const group = /<groupId>([^<]+)<\/groupId>/.exec(pom)?.[1] ?? '';
if (!group.startsWith('jp.golia')) {
  problems.push(`groupId ${group} is outside the namespace we can verify (jp.golia)`);
}

if (problems.length === 0) {
  console.log(
    `✓ ${group}:${/<artifactId>([^<]+)</.exec(pom)?.[1]}:${version} — ` +
      `aar, sources, javadoc and a POM Central accepts`,
  );
  process.exit(0);
}
for (const p of problems) console.error(`✗ ${p}`);
console.error(
  '\nCentral validates this at upload and rejects the bundle, which means\n' +
    're-staging a release rather than fixing one.',
);
process.exit(1);
