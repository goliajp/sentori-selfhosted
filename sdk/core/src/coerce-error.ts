/**
 * Turn anything thrown / rejected into a real `Error` instance so the
 * downstream pipeline can rely on `.name` / `.message` / `.stack`.
 *
 * Why this exists:
 *   `Promise.reject({foo: 1})`, `throw 'boom'`, `throw 42`, `throw null`
 *   and friends all surface through global error handlers. The naive
 *   `new Error(String(value))` collapses non-string non-Error inputs to
 *   the literal string "[object Object]" — completely useless to a
 *   triage user. This helper extracts the useful shape:
 *
 *     • Error instance               → returned as-is
 *     • string                       → `new Error(string)`
 *     • { name, message }            → `new Error(message)`, error.name = name
 *     • plain object / array         → `new Error(JSON-of-value)`, name='Error'
 *     • number / boolean / null / undefined / symbol / bigint
 *                                     → `new Error('Non-Error thrown: <repr>')`
 *
 *   Falls back to `String(value)` only when `JSON.stringify` throws
 *   (circular references, BigInt, etc.). In that case it tags the name
 *   `NonSerializableError` so it's obvious in the dashboard.
 *
 * Synchronous and dependency-free. Lives in core so every SDK package
 * (`@goliapkg/sentori-javascript`, `…-react`, `…-react-native`,
 * `…-vue`, `…-svelte`, `…-solid`) can share one implementation.
 */
export function coerceError(value: unknown): Error {
  if (value instanceof Error) return value

  if (typeof value === 'string') return new Error(value)

  if (typeof value === 'number' || typeof value === 'boolean') {
    return new Error(`Non-Error thrown: ${String(value)}`)
  }

  if (value === null) return new Error('Non-Error thrown: null')
  if (value === undefined) return new Error('Non-Error thrown: undefined')

  if (typeof value === 'symbol') {
    return new Error(`Non-Error thrown: ${value.toString()}`)
  }

  if (typeof value === 'bigint') {
    return new Error(`Non-Error thrown: ${value.toString()}n`)
  }

  // From here on the value is an object/array. Try to lift a sensible
  // name + message off it (common pattern for non-class errors), then
  // fall back to a JSON dump.
  const obj = value as Record<string, unknown>

  const namedMessage = typeof obj.message === 'string' ? obj.message : null
  let err: Error
  if (namedMessage) {
    err = new Error(namedMessage)
  } else {
    let serialized: string
    try {
      serialized = JSON.stringify(obj)
    } catch {
      const e = new Error(`Non-Error thrown: ${safeString(obj)}`)
      e.name = 'NonSerializableError'
      return e
    }
    err = new Error(serialized)
  }

  // Pick up `name` from the object if present and looks like a type tag.
  // Skip the trivial `'Object'` so the dashboard doesn't show it as
  // an exception type.
  const candidate = typeof obj.name === 'string' ? obj.name : null
  if (candidate && candidate !== 'Object' && candidate.length > 0 && candidate.length < 80) {
    err.name = candidate
  }

  return err
}

function safeString(v: unknown): string {
  try {
    return String(v)
  } catch {
    return '<unstringifiable value>'
  }
}
