// Unified symbolication-artifact upload: sourcemap / dsym / proguard
// all land on `POST /v1/releases/{release}/artifacts` (multipart
// `kind` + `file`), authenticated with an api-scope token. A late
// upload triggers retro-symbolication server-side, which is what
// makes the lenient exit-0 contract honest — nothing is lost
// forever.
//
// Uploads are gzipped on the wire (server ≥ 2.1.0 inflates
// transparently). Symbolication artifacts compress well — a plain
// R8 mapping ~10:1, DWARF ~3:1 — and the server's transport cap is
// smaller than its decompressed cap precisely so that the big ones
// (a real RN app's main dSYM runs hundreds of MB) fit as gzip.

import { readFileSync } from 'node:fs'
import { basename } from 'node:path'
import { gzipSync } from 'node:zlib'

export type UploadOpts = {
  apiUrl: string
  token: string
  release: string
  kind: 'dsym' | 'proguard' | 'sourcemap' | 'srcbundle'
  path: string
  /** Override the stored artifact name (defaults to the filename). */
  name?: string
}

/** Transport-side limit on the current server (256 MB). Used only to
 *  produce a useful error message — the server is the authority. */
const WIRE_LIMIT = 256 * 1024 * 1024

/** Gzip a payload for the wire (pass-through if it already is gzip),
 *  renaming `foo` → `foo.gz` to match. Shared by every artifact
 *  upload path so dSYM/mapping and sourcemap cannot drift. */
export function gzipForWire(
  bytes: Buffer | Uint8Array,
  name: string,
): { wire: Uint8Array; wireName: string } {
  const alreadyGz = bytes.length >= 2 && bytes[0] === 0x1f && bytes[1] === 0x8b
  const wire = alreadyGz ? new Uint8Array(bytes) : new Uint8Array(gzipSync(bytes))
  const wireName = alreadyGz || name.endsWith('.gz') ? name : `${name}.gz`
  if (wire.length > WIRE_LIMIT) {
    throw new Error(
      `${name} is ${Math.round(wire.length / 1024 / 1024)} MB gzipped — ` +
        `over the server's ${WIRE_LIMIT / 1024 / 1024} MB transport limit even compressed`,
    )
  }
  return { wire, wireName }
}

/** Exact-range copy into a standalone ArrayBuffer. `wire.buffer` is
 *  NOT safe here: Node pools small Buffers, so the backing buffer can
 *  be 16 KB of pool (offset view) — a Blob built on it would ship
 *  garbage bytes around the payload. */
export function wireArrayBuffer(wire: Uint8Array): ArrayBuffer {
  return wire.buffer.slice(wire.byteOffset, wire.byteOffset + wire.byteLength) as ArrayBuffer
}

export async function uploadArtifact(opts: UploadOpts): Promise<{ id: string }> {
  const bytes = readFileSync(opts.path)
  if (bytes.length === 0) throw new Error(`empty file: ${opts.path}`)

  // Already-compressed input (foo.map.gz) is passed through; the
  // server strips the .gz suffix from the stored name after inflating.
  const { wire, wireName } = gzipForWire(bytes, opts.name ?? basename(opts.path))

  const form = new FormData()
  form.append('kind', opts.kind)
  form.append('file', new Blob([wireArrayBuffer(wire)]), wireName)

  const url = `${opts.apiUrl.replace(/\/+$/, '')}/v1/releases/${encodeURIComponent(opts.release)}/artifacts`
  let resp: Response
  try {
    resp = await fetch(url, {
      method: 'POST',
      headers: { Authorization: `Bearer ${opts.token}` },
      body: form,
    })
  } catch (e) {
    // A proxy that hits its body-size cap tends to reset the
    // connection instead of answering, which surfaces here as a bare
    // network error. Say so — "fetch failed" cost one team a round of
    // debugging before they found the 413 underneath.
    throw new Error(
      `network error (${e instanceof Error ? e.message : String(e)}) — ` +
        `if the file is large this can be a proxy body-size cap rejecting ` +
        `the upload (${Math.round(wire.length / 1024 / 1024)} MB on the wire)`,
    )
  }
  if (!resp.ok) {
    const detail = await resp.text().catch(() => '')
    throw new Error(`${resp.status} ${resp.statusText}${detail ? ` — ${detail.slice(0, 200)}` : ''}`)
  }
  return (await resp.json()) as { id: string }
}
