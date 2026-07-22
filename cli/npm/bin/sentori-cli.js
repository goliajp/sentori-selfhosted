#!/usr/bin/env node
// Phase 17 sub-B: Node entry point for `npx @goliapkg/sentori-cli ...`.
// Forwards argv + stdio to the prebuilt Rust binary that postinstall
// dropped at ../vendor/sentori-cli.

'use strict'

const path = require('node:path')
const fs = require('node:fs')
const { spawn } = require('node:child_process')

const bin = path.join(__dirname, '..', 'vendor', 'sentori-cli')
if (!fs.existsSync(bin)) {
  console.error(
    'sentori-cli binary missing — postinstall did not complete. ' +
      'Try: `npm rebuild @goliapkg/sentori-cli` or set SENTORI_SKIP_DOWNLOAD=0 then reinstall.'
  )
  process.exit(127)
}

const child = spawn(bin, process.argv.slice(2), { stdio: 'inherit' })
child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal)
  } else {
    process.exit(code ?? 1)
  }
})
child.on('error', (e) => {
  console.error('failed to run sentori-cli:', e.message)
  process.exit(1)
})
