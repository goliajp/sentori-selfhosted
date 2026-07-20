// Phase 40 sub-E: in __DEV__, symbolicate a JS stack against the Metro
// dev server's /symbolicate endpoint before sending the event — the
// same thing RN's LogBox does. Release builds upload a source map and
// the server symbolicates at ingest; in dev there's no uploaded map,
// so without this dev errors arrive as `index.bundle:1:288432`.
//
// Best-effort and dev-only: any failure (Metro not running, timeout,
// bad response) leaves the stack untouched. Never throws.

import type { Frame, SentoriError } from '../types';

const TIMEOUT_MS = 2000;

/** Metro frame shape on the wire (note `lineNumber` / `methodName`). */
type MetroFrame = {
  collapse?: boolean;
  column?: null | number;
  file?: null | string;
  lineNumber?: null | number;
  methodName?: null | string;
};

/** Resolve `<devServer>/symbolicate`, or null if we're not running
 *  from a Metro dev server (release build, or not in RN).
 *
 *  Order matters:
 *    1. `react-native/Libraries/Core/Devtools/getDevServer` — the same
 *       helper LogBox + RN's own symbolicateStackTrace use. Works under
 *       both the legacy bridge and the new architecture (TurboModule),
 *       because internally it calls `NativeSourceCode.getConstants()`
 *       which is the correct path on new arch.
 *    2. `NativeModules.SourceCode.getConstants().scriptURL` — direct
 *       TurboModule fallback if (1) ever moves.
 *    3. `NativeModules.SourceCode.scriptURL` — legacy bridge (pre-new-
 *       arch RN). On new arch this property is `undefined` because
 *       constants aren't hoisted onto the module object — which is
 *       exactly the symptom Insight hit on RN 0.83 + new arch.
 */
function metroSymbolicateUrl(): null | string {
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const mod = require('react-native/Libraries/Core/Devtools/getDevServer') as {
      default?: () => { bundleLoadedFromServer: boolean; url: string };
    };
    const getDevServer = mod.default ?? (mod as unknown as () => { bundleLoadedFromServer: boolean; url: string });
    const ds = getDevServer();
    if (ds.bundleLoadedFromServer && typeof ds.url === 'string') {
      return ds.url.replace(/\/$/, '') + '/symbolicate';
    }
  } catch {
    // Older RN / non-RN runtime / path moved → fall through to NativeModules.
  }
  try {
    // eslint-disable-next-line @typescript-eslint/no-require-imports
    const rn = require('react-native') as {
      NativeModules?: {
        SourceCode?: {
          getConstants?: () => { scriptURL?: string };
          scriptURL?: string;
        };
      };
    };
    const sc = rn.NativeModules?.SourceCode;
    const scriptURL = sc?.scriptURL ?? sc?.getConstants?.()?.scriptURL;
    if (!scriptURL || !/^https?:\/\//.test(scriptURL)) return null;
    const u = new URL(scriptURL);
    return `${u.protocol}//${u.host}/symbolicate`;
  } catch {
    return null;
  }
}

function toMetroFrame(f: Frame): MetroFrame {
  return {
    column: f.column ?? 0,
    file: f.absolutePath ?? f.file,
    lineNumber: f.line,
    methodName: f.function ?? null,
  };
}

function fromMetroFrame(m: MetroFrame, fallback: Frame): Frame {
  // Metro couldn't resolve this frame (file null) → keep the original.
  if (!m.file) return fallback;
  return {
    absolutePath: m.file,
    column: typeof m.column === 'number' ? m.column : undefined,
    file: m.file,
    function: m.methodName ?? undefined,
    inApp: !m.collapse && !m.file.includes('node_modules'),
    line: typeof m.lineNumber === 'number' ? m.lineNumber : 0,
  };
}

/**
 * POST the frames to Metro's `/symbolicate` and return the mapped-back
 * stack, or `null` if it can't be done (not a dev server, Metro down,
 * timeout, malformed response). `url` is overridable for tests.
 */
export async function symbolicateStackViaMetro(
  frames: Frame[],
  opts: { url?: string } = {},
): Promise<Frame[] | null> {
  const url = opts.url ?? metroSymbolicateUrl();
  if (!url || frames.length === 0) return null;
  try {
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), TIMEOUT_MS);
    let resp: Response;
    try {
      resp = await fetch(url, {
        body: JSON.stringify({ stack: frames.map(toMetroFrame) }),
        headers: { 'Content-Type': 'application/json' },
        method: 'POST',
        signal: ctrl.signal,
      });
    } finally {
      clearTimeout(timer);
    }
    if (!resp.ok) return null;
    const body = (await resp.json()) as { stack?: MetroFrame[] };
    if (!Array.isArray(body.stack) || body.stack.length !== frames.length) return null;
    return body.stack.map((m, i) => fromMetroFrame(m, frames[i]!));
  } catch {
    return null;
  }
}

/** Replace `err.stack` (and the cause chain) in place with the
 *  Metro-symbolicated version, when possible. Never throws. */
export async function symbolicateErrorViaMetro(
  err: SentoriError,
  opts: { url?: string } = {},
): Promise<void> {
  const sym = await symbolicateStackViaMetro(err.stack, opts);
  if (sym) err.stack = sym;
  if (err.cause) await symbolicateErrorViaMetro(err.cause, opts);
}
