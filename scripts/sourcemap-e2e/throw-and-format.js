// Phase 16 sub-E helper for run.sh.
//
// Loads a bundled fixture in Node, captures the resulting Error's
// stack, and prints a Sentori event JSON to stdout.
//
// Usage:
//   node throw-and-format.js path/to/bundle.js release-name

const fs = require('node:fs')
const path = require('node:path')
const vm = require('node:vm')

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
  // `vm.runInThisContext` with an explicit filename, not `new Function`:
  // the latter attributes every frame to *this* file, so the stack came
  // back naming throw-and-format.js and there was nothing for a source
  // map to resolve. The point of the fixture is a stack that really
  // points into the bundle.
  vm.runInThisContext(code, { filename: bundlePath })
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
  // Only frames from inside the bundle; the Node frames that ran it are
  // not part of what the app would have reported.
  .filter(([, , file]) => file.endsWith(path.basename(bundlePath)))
  .map(([, fn, file, line, col]) => ({
    file: path.basename(file),
    line: Number(line),
    column: Number(col),
    function: fn || undefined,
    inApp: file.endsWith(path.basename(bundlePath)),
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
