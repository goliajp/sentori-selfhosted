// Phase 16 sub-E helper for run.sh.
//
// Loads a bundled fixture in Node, captures the resulting Error's
// stack, and prints a Sentori event JSON to stdout.
//
// Usage:
//   node throw-and-format.js path/to/bundle.js release-name

const fs = require('node:fs')
const path = require('node:path')

const [, , bundlePath, release] = process.argv
if (!bundlePath || !release) {
  console.error('usage: throw-and-format.js <bundle.js> <release>')
  process.exit(2)
}

const code = fs.readFileSync(bundlePath, 'utf8')
let stack = ''
let message = ''
let type = 'Error'
try {
  // eslint-disable-next-line no-new-func
  new Function(code)()
} catch (e) {
  stack = e.stack || ''
  message = e.message || 'unknown'
  type = e.constructor?.name || 'Error'
}

// Parse the stack into Sentori-shaped frames. We deliberately don't
// symbolicate here — the server does that on read after the .map
// upload — so frames carry the minified `bundle.js:1:colN` shape.
const frames = stack
  .split('\n')
  .slice(1)
  .map((line) => line.match(/at\s+(?:(\S+)\s+)?\(?([^:)]+):(\d+):(\d+)\)?/))
  .filter(Boolean)
  .map(([, fn, file, line, col]) => ({
    file: path.basename(file),
    line: Number(line),
    column: Number(col),
    function: fn || undefined,
    inApp: file.endsWith('bundle.js'),
  }))

const id = crypto.randomUUID()
const event = {
  id,
  timestamp: new Date().toISOString(),
  kind: 'error',
  platform: 'javascript',
  release,
  environment: 'sourcemap-e2e',
  device: { os: 'web', osVersion: process.version },
  app: { version: '1.0.0' },
  error: { type, message, stack: frames, cause: null },
}

console.log(JSON.stringify(event))
