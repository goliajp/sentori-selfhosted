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
export declare function coerceError(value: unknown): Error;
//# sourceMappingURL=coerce-error.d.ts.map