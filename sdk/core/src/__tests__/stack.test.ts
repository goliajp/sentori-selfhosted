import { expect, test } from 'bun:test'

import { parseStack } from '../stack.js'

test('parses V8 frames with parens', () => {
  const stack = `Error: boom
    at handle (file:///app/index.js:10:5)
    at run (/app/runner.ts:42:1)`
  const frames = parseStack(stack)
  expect(frames).toHaveLength(2)
  expect(frames[0]).toMatchObject({
    column: 5,
    function: 'handle',
    inApp: true,
    line: 10,
  })
  expect(frames[1]?.function).toBe('run')
})

test('parses SpiderMonkey @-style', () => {
  const stack = `boom@http://example.com/app.js:5:7
@http://example.com/app.js:1:1`
  const frames = parseStack(stack)
  expect(frames).toHaveLength(2)
  expect(frames[0]).toMatchObject({
    column: 7,
    function: 'boom',
    inApp: false, // http URL → not inApp
    line: 5,
  })
})

test('shortFilenames: strips protocol + path', () => {
  const stack = `at fn (https://cdn.example.com/static/App.tsx:1:1)`
  const frames = parseStack(stack, { shortFilenames: true })
  expect(frames[0]?.file).toBe('static/App.tsx')
})

test('inApp: node_modules and node:* are out', () => {
  const stack = `at fn (node:internal/process/task_queues:95:5)
at fn2 (/app/node_modules/react/index.js:1:1)
at fn3 (/app/src/main.ts:10:1)`
  const frames = parseStack(stack)
  expect(frames.map((f) => f.inApp)).toEqual([false, false, true])
})

test('Hermes bytecode frames: strips the "address at" marker', () => {
  const stack = `RuntimeError: undefined is not an object
    at handleSubmit (address at /var/containers/Bundle/Application/X/MyApp.app/main.jsbundle:1:289430)
    at apply (native)
    at _callee$ (address at /var/containers/Bundle/Application/X/MyApp.app/main.jsbundle:1:120015)`
  const frames = parseStack(stack)
  expect(frames).toHaveLength(2) // (native) dropped — no location
  expect(frames[0]).toMatchObject({
    column: 289430,
    file: '/var/containers/Bundle/Application/X/MyApp.app/main.jsbundle',
    function: 'handleSubmit',
    inApp: true,
    line: 1,
  })
  expect(frames[1]?.function).toBe('_callee$')
  expect(frames[1]?.column).toBe(120015)
})

test('Hermes dev (Metro) frames: fn@http://host/index.bundle:line:col', () => {
  const stack = `handleSubmit@http://localhost:8081/index.bundle?platform=ios&dev=true:1:289430
onPress@http://localhost:8081/index.bundle?platform=ios&dev=true:1:120015`
  const frames = parseStack(stack)
  expect(frames).toHaveLength(2)
  expect(frames[0]).toMatchObject({ column: 289430, function: 'handleSubmit', line: 1 })
  expect(frames[0]?.file).toContain('index.bundle')
  expect(frames[0]?.inApp).toBe(true) // the dev bundle IS the app
})

test('empty / non-string returns []', () => {
  expect(parseStack(undefined)).toEqual([])
  expect(parseStack('')).toEqual([])
  expect(parseStack('Error: just a header')).toEqual([])
})

test('inApp: a Metro dev-server bundle URL is the app, not a library', () => {
  // Real shape from an Expo dev client: the whole app is served from
  // a URL whose path runs through node_modules/expo-router.
  const url =
    'http://192.168.50.20:8081/node_modules/expo-router/entry.bundle//&platform=ios&dev=true&hot=false'
  const stack = `Error: boom
    at triggerSentoriError (${url}:721724:58)
    at anonymous (${url}:100:1)`
  const frames = parseStack(stack)
  expect(frames).toHaveLength(2)
  expect(frames.every((f) => f.inApp)).toBe(true)
})

test('inApp: plain remote scripts stay out', () => {
  const frames = parseStack(`at fn (https://cdn.example.com/analytics.js:1:1)`)
  expect(frames[0]?.inApp).toBe(false)
})
