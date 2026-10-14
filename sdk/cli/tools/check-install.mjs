#!/usr/bin/env node
// Test the consumer artifact, including executable links. Optional registry spec
// is used after publishing; default packs the already-built working tree.
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
const pkg = resolve(dirname(fileURLToPath(import.meta.url)), '..');
assert.equal(readFileSync(resolve(pkg, '../../docs/cli.md'), 'utf8'),
  readFileSync(join(pkg, 'README.md'), 'utf8').replaceAll('../../LICENSE-', '../LICENSE-'),
  'Published CLI README and /docs/cli must describe the same workflow');
const version = JSON.parse(readFileSync(join(pkg, 'package.json'))).version;
const root = mkdtempSync(join(tmpdir(), 'sentori-cli-install-'));
const env = Object.fromEntries(Object.entries(process.env).filter(([name]) => !name.startsWith('SENTORI_')));
function run(command, args, expected = 0) {
  const r = spawnSync(command, args, { cwd: root, env, encoding: 'utf8', timeout: 180_000 });
  assert.equal(r.status, expected, `${command} ${args.join(' ')}\n${r.error ?? ''}\n${r.stdout}\n${r.stderr}`);
  return r.stdout;
}
function cli(...args) { return run('npx', ['--no-install', 'sentori', ...args]); }
try {
  writeFileSync(join(root, 'package.json'), '{"name":"sentori-install-check","private":true}');
  let spec = process.argv[2];
  if (!spec) {
    const result = JSON.parse(run('npm', ['pack', pkg, '--ignore-scripts', '--json', '--pack-destination', root]));
    const entries = Array.isArray(result) ? result : Object.values(result);
    assert.equal(entries.length, 1, 'npm pack must produce exactly one package');
    spec = join(root, entries[0].filename);
  }
  run('npm', ['install', '--ignore-scripts', '--no-audit', '--no-fund', '--save-exact', spec]);
  assert.equal(cli('--version').trim(), `sentori ${version}`);
  assert.equal(run('npx', ['--no-install', 'sentori-cli', '--version']).trim(), `sentori ${version}`);
  assert.match(cli('--help'), /upload all/);
  mkdirSync(join(root, 'src'));
  writeFileSync(join(root, 'src/app.ts'), 'export const value = 42\n');
  writeFileSync(join(root, 'app.map'), JSON.stringify({ version: 3, sources: ['src/app.ts'], names: [], mappings: 'AAAA' }));
  cli('init', '--api-url', 'http://127.0.0.1:1', '--release', 'install-check@1+1', '--sources', 'src', '--sourcemap', 'app.map');
  cli('config', 'validate');
  cli('upload', 'all', '--dry-run');
  cli('sources', 'pack', '--root', '.', '--release', 'install-check@1+1', '--output', 'sources.zip', 'src');
  assert.equal(readFileSync(join(root, 'sources.zip')).subarray(0, 2).toString(), 'PK');
  // With no credential, these gates must fail rather than report success.
  run('npx', ['--no-install', 'sentori', 'doctor'], 1);
  run('npx', ['--no-install', 'sentori', 'verify'], 1);
  console.log(`PASS installed CLI ${version}: both bins, init, validate, dry-run, source ZIP and failing verification gates`);
} finally { rmSync(root, { recursive: true, force: true }); }
