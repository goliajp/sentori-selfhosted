#!/usr/bin/env node
// Compatibility build hook. The sentori CLI owns packing, upload and verification.
'use strict'
const { spawnSync } = require('node:child_process')
const { existsSync, readFileSync } = require('node:fs')
const path = require('node:path')
const { parseArgs } = require('node:util')
const strict = process.argv.includes('--strict')
function fail(message) {
  console.error(`[sentori] ${message}\nRetry: sentori upload all --config sentori.config.json; sentori verify`)
  process.exit(strict ? 1 : 0)
}
try {
  const { values } = parseArgs({ options: {
    platform: { type: 'string' }, release: { type: 'string' }, config: { type: 'string' },
    'git-tag': { type: 'string' }, 'api-url': { type: 'string' }, token: { type: 'string' },
    project: { type: 'string' }, strict: { type: 'boolean' }, help: { type: 'boolean', short: 'h' },
  } })
  if (values.help) {
    console.log('sentori-rn-upload-source-bundle [--config sentori.config.json] [--release <exact-sdk-release>] [--git-tag <tag>] [--strict]\nCreate a unified upload configuration with sentori init. Legacy sources.ios/android configs accept --platform ios|android.')
    process.exit(0)
  }
  const cwd = process.cwd()
  const configPath = path.resolve(values.config || 'sentori.config.json')
  const config = existsSync(configPath) ? JSON.parse(readFileSync(configPath, 'utf8')) : {}
  const args = []
  if (config.version === 1) {
    args.push('upload', 'all', '--config', configPath)
    if (values.release) args.push('--release', values.release)
    if (values['api-url']) args.push('--api-url', values['api-url'])
  } else {
    const platform = values.platform
    if (!['ios', 'android'].includes(platform)) fail('legacy configuration requires --platform ios|android')
    const release = values.release || process.env.SENTORI_RELEASE
    if (!release) fail('set the exact SDK release with --release or SENTORI_RELEASE; package version alone may identify the wrong build')
    args.push('upload', 'srcbundle', '--release', release, '--root', cwd)
    const apiUrl = values['api-url'] || process.env.SENTORI_API_URL || config.apiUrl
    if (apiUrl) args.push('--api-url', apiUrl)
    args.push(config.sources?.[platform] || (platform === 'ios' ? 'ios' : 'android/app/src'))
  }
  if (values['git-tag']) args.push('--git-tag', values['git-tag'])
  if (strict) args.push('--strict')
  const local = path.join(cwd, 'node_modules', '.bin', 'sentori')
  const bin = existsSync(local) ? local : 'sentori'
  const env = { ...process.env }
  const token = values.token || process.env.SENTORI_TOKEN || config.token
  if (token) env[config.version === 1 ? config.tokenEnv || 'SENTORI_TOKEN' : 'SENTORI_TOKEN'] = token
  const result = spawnSync(bin, args, { stdio: 'inherit', env })
  if (result.error || result.status !== 0) fail(result.error?.message || `sentori exited ${result.status}`)
} catch (error) { fail(error.message) }
