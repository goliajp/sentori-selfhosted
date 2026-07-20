// v0.9.0 #14 — `sentori.measureFn(name, fn)`. Profile-lite. Wrap an
// async (or sync) function call in a span so it shows on the issue
// detail trace waterfall without writing the boilerplate every time.
// The full Hermes-sampler profiler (#4) is the deep version of this
// idea; `measureFn` is the cheap version that doesn't need a native
// module.

import { startSpan } from '@goliapkg/sentori-core';

export async function measureFn<T>(
  name: string,
  fn: () => Promise<T> | T,
  opts?: { tags?: Record<string, string> },
): Promise<T> {
  const span = startSpan('sentori.measureFn', {
    name,
    tags: opts?.tags ?? {},
  });
  try {
    const result = await fn();
    span.finish({ status: 'ok' });
    return result;
  } catch (e) {
    if (e instanceof Error) span.setTag('error.message', e.message);
    span.finish({ status: 'error' });
    throw e;
  }
}
