import { afterEach, beforeEach, expect, test } from 'bun:test'
import { spawnSync } from 'node:child_process'
import { mkdtempSync, mkdirSync, readFileSync, realpathSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'

const hook = fileURLToPath(new URL('../../../react-native/bin/sentori-rn-upload-source-bundle.cjs', import.meta.url))
let root: string
beforeEach(() => {
  root = realpathSync(mkdtempSync(join(tmpdir(), 'sentori-hook-test-')))
  mkdirSync(join(root, 'node_modules/.bin'), { recursive: true })
  writeFileSync(join(root, 'node_modules/.bin/sentori'), `#!/usr/bin/env node
require('node:fs').writeFileSync('invocation.json', JSON.stringify({args:process.argv.slice(2),token:process.env.BUILD_TOKEN}));process.exit(Number(process.env.STUB_EXIT || 0));`, { mode: 0o755 })
})
afterEach(() => rmSync(root, { recursive: true, force: true }))
function run(args: string[], extra: Record<string,string> = {}) {
  const env = { ...process.env, ...extra }; delete env.SENTORI_RELEASE
  return spawnSync('node', [hook, ...args], { cwd: root, env, encoding: 'utf8' })
}
test('RN hook delegates the entire configured upload to sentori and keeps credentials out of argv', () => {
  writeFileSync(join(root, 'sentori.config.json'), JSON.stringify({ version: 1, tokenEnv: 'BUILD_TOKEN' }))
  expect(run(['--release', 'app@1+7', '--git-tag', 'v1', '--token', 'secret', '--strict']).status).toBe(0)
  const invocation = JSON.parse(readFileSync(join(root, 'invocation.json'), 'utf8'))
  expect(invocation.args).toEqual(['upload', 'all', '--config', join(root, 'sentori.config.json'), '--release', 'app@1+7', '--git-tag', 'v1', '--strict'])
  expect(invocation.token).toBe('secret')
  expect(run([], { STUB_EXIT: '1' }).status).toBe(0)
  expect(run(['--strict'], { STUB_EXIT: '1' }).status).toBe(1)
})
test('legacy hook requires exact release identity instead of guessing package version', () => {
  writeFileSync(join(root, 'package.json'), JSON.stringify({ version: '1.0.0' }))
  expect(run(['--platform', 'ios', '--strict']).status).toBe(1)
  expect(run(['--platform', 'ios', '--release', 'app@1+7', '--strict']).status).toBe(0)
  expect(JSON.parse(readFileSync(join(root, 'invocation.json'), 'utf8')).args).toEqual(['upload', 'srcbundle', '--release', 'app@1+7', '--root', root, 'ios', '--strict'])
})
