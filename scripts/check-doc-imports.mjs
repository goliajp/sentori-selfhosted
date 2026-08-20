#!/usr/bin/env node
// Every symbol the native docs tell you to import exists.
//
// `docs/sdk-kotlin.md` showed a Gradle coordinate and no import line
// at all. The groupId is `jp.golia.sentori`; the package is
// `com.sentori`. An integrator guessed the obvious thing, got
// `Unresolved reference 'jp'` on a dependency that had resolved
// perfectly, and went looking at `mavenCentral()` and transitive
// dependencies — because a resolution failure and an import failure
// look nothing alike, and only one of them was happening.
//
// Nothing catches this. The docs are prose, the package name is in
// Kotlin source, and the coordinate is in Gradle. The only place they
// meet is in somebody's editor.
//
//   node scripts/check-doc-imports.mjs
//
// Checks two things per doc: that it shows an import at all, and that
// each imported symbol is declared in the package it names.

import { readFileSync, readdirSync, statSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');

const DOCS = [
  {
    doc: 'docs/sdk-kotlin.md',
    src: 'sdk/native/android/src/main/java/com/sentori',
    ext: '.kt',
    // `package com.sentori`
    pkgOf: (s) => /^package\s+([\w.]+)/m.exec(s)?.[1],
    // `class Sentori`, `object SentoriConfig`, `data class …`
    declOf: (s) => [
      ...s.matchAll(/^(?:public\s+|internal\s+)?(?:open\s+|abstract\s+|data\s+|sealed\s+)*(?:class|object|interface|enum class)\s+(\w+)/gm),
    ].map((m) => m[1]),
    importRe: /^import\s+([\w.]+)\.(\w+)$/gm,
  },
];

const problems = [];

function walk(dir, ext, out = []) {
  for (const e of readdirSync(dir)) {
    const p = join(dir, e);
    if (statSync(p).isDirectory()) walk(p, ext, out);
    else if (p.endsWith(ext)) out.push(p);
  }
  return out;
}

for (const spec of DOCS) {
  const doc = readFileSync(join(root, spec.doc), 'utf8');

  // What the package actually declares.
  const declared = new Map(); // package → Set<symbol>
  for (const file of walk(join(root, spec.src), spec.ext)) {
    const src = readFileSync(file, 'utf8');
    const pkg = spec.pkgOf(src);
    if (!pkg) continue;
    if (!declared.has(pkg)) declared.set(pkg, new Set());
    for (const name of spec.declOf(src)) declared.get(pkg).add(name);
  }
  if (declared.size === 0) {
    problems.push(`read no packages out of ${spec.src} — this check would pass on anything`);
    continue;
  }

  const imports = [...doc.matchAll(spec.importRe)];
  if (imports.length === 0) {
    problems.push(
      `${spec.doc} shows no import at all. The groupId is not the package, and a ` +
        'reader with only a Gradle coordinate has to guess.',
    );
    continue;
  }

  for (const [line, pkg, symbol] of imports) {
    const known = declared.get(pkg);
    if (!known) {
      problems.push(
        `${spec.doc}: \`${line.trim()}\` names package '${pkg}', which ${spec.src} ` +
          `does not declare (it has ${[...declared.keys()].join(', ')})`,
      );
      continue;
    }
    if (!known.has(symbol)) {
      problems.push(
        `${spec.doc}: \`${line.trim()}\` — '${pkg}' declares no '${symbol}'`,
      );
    }
  }
}

if (problems.length === 0) {
  console.log(`✓ ${DOCS.length} native doc(s): every import shown resolves to real code`);
  process.exit(0);
}
for (const p of problems) console.error(`✗ ${p}`);
console.error(
  '\nAn import that does not resolve reads as a broken dependency, not a typo,\n' +
    'and costs the reader an afternoon in the wrong file.',
);
process.exit(1);
