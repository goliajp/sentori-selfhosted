#!/usr/bin/env node
// Phase 17 sub-B: download the prebuilt sentori-cli binary that
// matches the current platform / arch and place it at ../vendor/.
//
// Skips if SENTORI_SKIP_DOWNLOAD=1 (CI, monorepo bootstrap, etc.).

'use strict'

const fs = require('node:fs')
const https = require('node:https')
const path = require('node:path')
const { spawnSync } = require('node:child_process')

const pkg = require(path.join(__dirname, '..', 'package.json'))
const TAG = `cli-v${pkg.version}`

const SUPPORTED = {
  'linux-x64': 'linux-x64',
  'linux-arm64': 'linux-arm64',
  'darwin-x64': 'darwin-x64',
  'darwin-arm64': 'darwin-arm64',
}

function detect() {
  return `${process.platform}-${process.arch}`
}

function fetch(url, destStream) {
  return new Promise((resolve, reject) => {
    const handle = (u) => {
      https
        .get(u, { headers: { 'user-agent': `sentori-cli-installer/${pkg.version}` } }, (res) => {
          const code = res.statusCode || 0
          if (code >= 300 && code < 400 && res.headers.location) {
            return handle(res.headers.location)
          }
          if (code !== 200) {
            return reject(new Error(`HTTP ${code} from ${u}`))
          }
          res.pipe(destStream)
          destStream.on('finish', resolve)
          destStream.on('error', reject)
        })
        .on('error', reject)
    }
    handle(url)
  })
}

async function main() {
  if (process.env.SENTORI_SKIP_DOWNLOAD === '1') {
    console.log('SENTORI_SKIP_DOWNLOAD=1; skipping sentori-cli binary download')
    return
  }

  const key = detect()
  const slug = SUPPORTED[key]
  if (!slug) {
    console.error(
      `sentori-cli has no prebuilt binary for ${key}. ` +
        `Cargo install instead: cargo install --git https://github.com/goliajp/sentori --bin sentori-cli`
    )
    process.exit(0) // soft fail — user can still cargo install
  }

  const filename = `sentori-cli-${TAG}-${slug}.tar.gz`
  const url = `https://github.com/goliajp/sentori/releases/download/${TAG}/${filename}`
  const vendorDir = path.join(__dirname, '..', 'vendor')
  fs.mkdirSync(vendorDir, { recursive: true })
  const tarball = path.join(vendorDir, 'sentori-cli.tar.gz')

  console.log(`sentori-cli ${pkg.version}: downloading ${slug} from GitHub Release`)
  await fetch(url, fs.createWriteStream(tarball))

  const tar = spawnSync('tar', ['-xzf', tarball, '-C', vendorDir], { stdio: 'inherit' })
  if (tar.status !== 0) {
    console.error('tar -xzf failed; is `tar` on PATH?')
    process.exit(1)
  }
  fs.unlinkSync(tarball)

  const bin = path.join(vendorDir, 'sentori-cli')
  fs.chmodSync(bin, 0o755)
  console.log(`✓ sentori-cli ${pkg.version} ready (${slug})`)
}

main().catch((e) => {
  console.error('sentori-cli install failed:', e.message)
  process.exit(1)
})
