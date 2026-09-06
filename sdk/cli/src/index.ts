#!/usr/bin/env node
import { parseArgs } from 'node:util'

import {
  fetchBundle,
  formatIssueLine,
  listIssues,
  noteIssue,
  resolveIssue,
} from './issue.js'
import { isStrict, lenientFail, stripStrict } from './lenient.js'
import { runMcpServer } from './mcp.js'
import { uploadDsym, uploadMapping } from './native-artifacts.js'
import { scanProbes, syncProbes } from './probes.js'
import {
  parseJsonArg,
  pushCredsDelete,
  pushCredsList,
  pushCredsSet,
  pushReceipt,
  pushSend,
} from './push.js'
import { reactNativeUpload } from './react-native.js'
import { fetchArtifacts } from './artifacts.js'
import { uploadArtifact } from './upload.js'

const HELP = `sentori-cli — Sentori command-line interface

Symbolication artifacts (api-scope token; failures NEVER block your
build — exit 0 with a friendly note unless --strict):
  sentori-cli upload sourcemap --release <r> --token <t> <path...>
  sentori-cli upload dsym      --release <r> --token <t> <path.dSYM>
  sentori-cli upload mapping   --release <r> --token <t> mapping.txt
                               (mapping = the R8/proguard map; stored as kind "proguard")
  sentori-cli upload srcbundle --release <r> --token <t> <src-dir...>
                               (native source files so dSYM/proguard frames
                                show the failing line; nothing touches git)
  sentori-cli react-native upload --release <r> --token <t> \\
      --metro-map <m> --hermes-map <h> [--bundle <b>]

Release gate (the ONE command here that is allowed to fail):
  sentori-cli artifacts check --release <r> --token <t> [--expect dsym,sourcemap]
      Asks the server what actually landed for this release. With
      --expect, exits 1 when one of those kinds is missing. Uploads
      never block your build; this is the step that does, and it
      catches the case a local "we ran the upload" ledger cannot:
      the upload step that stopped being called at all.

Regression tripwires (design: probes):
  sentori-cli probes sync --release <r> --token <t> [--dir .]
      Statically scans source for sentori.probe('REF') call sites and
      registers them, so a silent probe is visibly alive.

CI triage (the same /api surface an AI agent uses):
  sentori-cli issue list [--status open] [--kind error]
  sentori-cli issue resolve <issue-id> [--in-release <r>]
  sentori-cli issue note <issue-id> --body "fixed in abc123"
  sentori-cli issue bundle <issue-id>

MCP (for Claude Code and friends):
  sentori-cli mcp serve --token <api-token> [--api-url <url>]

Push (carried):
  sentori-cli push send / receipt / creds ...

Common options:
  --token       api-scope token (or $SENTORI_TOKEN)
  --api-url     instance URL (or $SENTORI_API_URL; default https://sentori.golia.jp)
  --strict      upload commands: exit non-zero on failure
`

type Common = { apiUrl: string; release: string; token: string }

function parseCommon(values: Record<string, unknown>): Common | null {
  const release = typeof values.release === 'string' ? values.release : undefined
  if (!release) {
    console.error("error: --release is required (must match the SDK's init({ release }))")
    return null
  }
  const token =
    (typeof values.token === 'string' ? values.token : undefined) ?? process.env.SENTORI_TOKEN
  if (!token) {
    console.error('error: --token (or $SENTORI_TOKEN) is required')
    return null
  }
  const apiUrl =
    (typeof values['api-url'] === 'string' ? values['api-url'] : undefined) ??
    (typeof values['ingest-url'] === 'string' ? values['ingest-url'] : undefined) ??
    process.env.SENTORI_API_URL ??
    'https://sentori.golia.jp'
  return { apiUrl, release, token }
}

type ApiOnly = { apiUrl: string; token: string }

function parseApiCfg(values: Record<string, unknown>): ApiOnly | null {
  const token =
    (typeof values.token === 'string' ? values.token : undefined) ??
    process.env.SENTORI_ADMIN_TOKEN ??
    process.env.SENTORI_TOKEN
  if (!token) {
    console.error('error: --token (or $SENTORI_TOKEN) is required')
    return null
  }
  const apiUrl =
    (typeof values['api-url'] === 'string' ? values['api-url'] : undefined) ??
    (typeof values['ingest-url'] === 'string' ? values['ingest-url'] : undefined) ??
    process.env.SENTORI_API_URL ??
    'https://sentori.golia.jp'
  return { apiUrl, token }
}

// Kept for the push commands, which carried over unchanged and only
// need url+token (projectId rides in their own paths).
type AdminCfg = { apiUrl: string; projectId: string; token: string }

function parseAdminCfg(values: Record<string, unknown>): AdminCfg | null {
  const projectId =
    (typeof values.project === 'string' ? values.project : undefined) ??
    process.env.SENTORI_PROJECT_ID
  if (!projectId) {
    console.error('error: --project <uuid> (or $SENTORI_PROJECT_ID) is required')
    return null
  }
  const api = parseApiCfg(values)
  if (!api) return null
  return { apiUrl: api.apiUrl, projectId, token: api.token }
}

const UPLOAD_OPTS = {
  'api-url': { type: 'string' },
  help: { short: 'h', type: 'boolean' },
  'ingest-url': { type: 'string' },
  release: { type: 'string' },
  token: { type: 'string' },
} as const

// ── upload commands (lenient by contract) ─────────────────────────

async function cmdUploadSrcbundle(argv: string[]): Promise<number> {
  const strict = isStrict(argv)
  let parsed
  try {
    parsed = parseArgs({ allowPositionals: true, args: stripStrict(argv), options: UPLOAD_OPTS })
  } catch (e) {
    console.error(`error: ${(e as Error).message}\n${HELP}`)
    return 2
  }
  if (parsed.values.help) {
    console.log(HELP)
    return 0
  }
  const c = parseCommon(parsed.values)
  if (!c) return 2
  if (parsed.positionals.length === 0) {
    console.error('error: at least one source directory is required')
    return 2
  }
  try {
    const { collectSources } = await import('./srcbundle.js')
    const { bundle, stats } = collectSources(parsed.positionals)
    if (stats.files === 0) {
      console.error('error: no native source files found under the given directories')
      return 2
    }
    const { writeFileSync, mkdtempSync } = await import('node:fs')
    const { tmpdir } = await import('node:os')
    const { join } = await import('node:path')
    const tmp = join(mkdtempSync(join(tmpdir(), 'sentori-srcbundle-')), 'srcbundle.json')
    writeFileSync(tmp, JSON.stringify(bundle))
    await uploadArtifact({ ...c, kind: 'srcbundle', path: tmp, name: 'srcbundle.json' })
    console.log(
      `uploaded a source bundle for "${c.release}" — ${stats.files} file(s)` +
        `${stats.skipped ? ` (${stats.skipped} skipped as oversized/unreadable)` : ''}; ` +
        `native frames on this release now show their source window.`,
    )
    return 0
  } catch (e) {
    return lenientFail(strict, {
      failure: `srcbundle upload failed (${(e as Error).message})`,
      impact: `native frames from ${c.release} will show file:line without the code window until uploaded.`,
      retry: `sentori-cli upload srcbundle --release "${c.release}" --token <t> ${parsed.positionals.join(' ')}`,
    })
  }
}

async function cmdUploadSourcemap(argv: string[]): Promise<number> {
  const strict = isStrict(argv)
  let parsed
  try {
    parsed = parseArgs({ allowPositionals: true, args: stripStrict(argv), options: UPLOAD_OPTS })
  } catch (e) {
    console.error(`error: ${(e as Error).message}\n${HELP}`)
    return 2
  }
  if (parsed.values.help) {
    console.log(HELP)
    return 0
  }
  const c = parseCommon(parsed.values)
  if (!c) return 2
  if (parsed.positionals.length === 0) {
    console.error('error: at least one sourcemap path is required')
    return 2
  }
  try {
    for (const p of parsed.positionals) {
      await uploadArtifact({ ...c, kind: 'sourcemap', path: p })
    }
    console.log(
      `uploaded ${parsed.positionals.length} sourcemap(s) for "${c.release}" — minified stacks on this release now resolve to source.`,
    )
    return 0
  } catch (e) {
    return lenientFail(strict, {
      failure: `sourcemap upload failed (${(e as Error).message})`,
      impact: `crashes from ${c.release} will show minified stacks until the map is uploaded.`,
      retry: `sentori-cli upload sourcemap --release "${c.release}" --token <t> ${parsed.positionals.join(' ')}`,
    })
  }
}

async function cmdUploadDsym(argv: string[]): Promise<number> {
  const strict = isStrict(argv)
  let parsed
  try {
    parsed = parseArgs({
      allowPositionals: true,
      args: stripStrict(argv),
      options: {
        ...UPLOAD_OPTS,
        arch: { type: 'string' },
        'debug-id': { type: 'string' },
        'object-name': { type: 'string' },
      },
    })
  } catch (e) {
    console.error(`error: ${(e as Error).message}\n${HELP}`)
    return 2
  }
  if (parsed.values.help) {
    console.log(HELP)
    return 0
  }
  const c = parseCommon(parsed.values)
  if (!c) return 2
  const path = parsed.positionals[0]
  if (!path) {
    console.error('error: a path to a .dSYM bundle or DWARF binary is required')
    return 2
  }
  const debugId = parsed.values['debug-id']
  const arch = parsed.values.arch
  if ((debugId && !arch) || (arch && !debugId)) {
    console.error('error: --debug-id and --arch must be passed together (or both omitted)')
    return 2
  }
  try {
    const r = await uploadDsym({
      apiUrl: c.apiUrl,
      arch: typeof arch === 'string' ? arch : undefined,
      debugId: typeof debugId === 'string' ? debugId : undefined,
      objectName:
        typeof parsed.values['object-name'] === 'string'
          ? parsed.values['object-name']
          : undefined,
      path,
      release: c.release,
      token: c.token,
    })
    console.log(`uploaded ${r.slices.length} dSYM slice(s):`)
    for (const s of r.slices) console.log(`  ${s.debugId}  (${s.arch})`)
    return 0
  } catch (e) {
    return lenientFail(strict, {
      failure: `dSYM upload failed (${(e as Error).message})`,
      impact: `native iOS stacks from ${c.release} stay unsymbolicated until the dSYM lands.`,
      retry: `sentori-cli upload dsym --release "${c.release}" --token <t> ${path}`,
    })
  }
}

async function cmdUploadMapping(argv: string[]): Promise<number> {
  const strict = isStrict(argv)
  let parsed
  try {
    parsed = parseArgs({
      allowPositionals: true,
      args: stripStrict(argv),
      options: { ...UPLOAD_OPTS, 'debug-id': { type: 'string' } },
    })
  } catch (e) {
    console.error(`error: ${(e as Error).message}\n${HELP}`)
    return 2
  }
  if (parsed.values.help) {
    console.log(HELP)
    return 0
  }
  const c = parseCommon(parsed.values)
  if (!c) return 2
  const path = parsed.positionals[0]
  if (!path) {
    console.error('error: a path to mapping.txt is required')
    return 2
  }
  try {
    await uploadMapping({
      apiUrl: c.apiUrl,
      debugId:
        typeof parsed.values['debug-id'] === 'string' ? parsed.values['debug-id'] : undefined,
      path,
      release: c.release,
      token: c.token,
    })
    console.log(`uploaded mapping for "${c.release}" — R8 names on this release now demangle.`)
    return 0
  } catch (e) {
    return lenientFail(strict, {
      failure: `mapping upload failed (${(e as Error).message})`,
      impact: `Android stacks from ${c.release} stay R8-obfuscated until the mapping lands.`,
      retry: `sentori-cli upload mapping --release "${c.release}" --token <t> ${path}`,
    })
  }
}

async function cmdReactNativeUpload(argv: string[]): Promise<number> {
  const strict = isStrict(argv)
  let parsed
  try {
    parsed = parseArgs({
      args: stripStrict(argv),
      options: {
        ...UPLOAD_OPTS,
        bundle: { type: 'string' },
        'dry-run': { type: 'boolean' },
        'hermes-map': { type: 'string' },
        'metro-map': { type: 'string' },
      },
    })
  } catch (e) {
    console.error(`error: ${(e as Error).message}\n${HELP}`)
    return 2
  }
  if (parsed.values.help) {
    console.log(HELP)
    return 0
  }
  const c = parseCommon(parsed.values)
  if (!c) return 2
  const metroMap = parsed.values['metro-map']
  const hermesMap = parsed.values['hermes-map']
  if (typeof metroMap !== 'string' || typeof hermesMap !== 'string') {
    console.error('error: --metro-map and --hermes-map are both required')
    return 2
  }
  try {
    const result = await reactNativeUpload({
      apiUrl: c.apiUrl,
      bundle: typeof parsed.values.bundle === 'string' ? parsed.values.bundle : undefined,
      dryRun: parsed.values['dry-run'] === true,
      hermesMap,
      metroMap,
      release: c.release,
      token: c.token,
    })
    console.log(`uploaded ${result.uploaded ?? result.files.length} file(s) for "${c.release}".`)
    return 0
  } catch (e) {
    return lenientFail(strict, {
      failure: `react-native upload failed (${(e as Error).message})`,
      impact: `Hermes stacks from ${c.release} stay unsymbolicated until the composed map lands.`,
      retry: `sentori-cli react-native upload --release "${c.release}" --token <t> --metro-map ${metroMap} --hermes-map ${hermesMap}`,
    })
  }
}

// ── probes sync ───────────────────────────────────────────────────

async function cmdProbesSync(argv: string[]): Promise<number> {
  const strict = isStrict(argv)
  let parsed
  try {
    parsed = parseArgs({
      args: stripStrict(argv),
      options: { ...UPLOAD_OPTS, dir: { type: 'string' } },
    })
  } catch (e) {
    console.error(`error: ${(e as Error).message}\n${HELP}`)
    return 2
  }
  if (parsed.values.help) {
    console.log(HELP)
    return 0
  }
  const c = parseCommon(parsed.values)
  if (!c) return 2
  const dir = typeof parsed.values.dir === 'string' ? parsed.values.dir : '.'
  const refs = scanProbes(dir)
  if (refs.length === 0) {
    console.log(`no sentori.probe() call sites found under ${dir} — nothing to register.`)
    return 0
  }
  try {
    const r = await syncProbes({ apiUrl: c.apiUrl, token: c.token, release: c.release, refs })
    console.log(`registered ${r.registered} probe(s) for "${c.release}": ${refs.join(', ')}`)
    return 0
  } catch (e) {
    return lenientFail(strict, {
      failure: `probes sync failed (${(e as Error).message})`,
      impact: `silent probes on ${c.release} can't be told apart from deleted code until registered.`,
      retry: `sentori-cli probes sync --release "${c.release}" --token <t> --dir ${dir}`,
    })
  }
}

// ── issue commands (the /api surface) ─────────────────────────────

const ISSUE_OPTS = {
  'api-url': { type: 'string' },
  help: { short: 'h', type: 'boolean' },
  'ingest-url': { type: 'string' },
  token: { type: 'string' },
} as const

async function cmdIssueList(argv: string[]): Promise<number> {
  let parsed
  try {
    parsed = parseArgs({
      args: argv,
      options: { ...ISSUE_OPTS, kind: { type: 'string' }, status: { type: 'string' } },
    })
  } catch (e) {
    console.error(`error: ${(e as Error).message}\n${HELP}`)
    return 2
  }
  if (parsed.values.help) {
    console.log(HELP)
    return 0
  }
  const cfg = parseApiCfg(parsed.values)
  if (!cfg) return 2
  try {
    const rows = await listIssues(cfg, {
      kind: parsed.values.kind as string | undefined,
      status: (parsed.values.status as string | undefined) ?? 'open',
    })
    if (rows.length === 0) {
      console.log('(no matching issues)')
      return 0
    }
    for (const r of rows) console.log(formatIssueLine(r))
    return 0
  } catch (e) {
    console.error(`issue list failed: ${(e as Error).message}`)
    return 1
  }
}

async function cmdIssueResolve(argv: string[]): Promise<number> {
  let parsed
  try {
    parsed = parseArgs({
      allowPositionals: true,
      args: argv,
      options: { ...ISSUE_OPTS, 'in-release': { type: 'string' } },
    })
  } catch (e) {
    console.error(`error: ${(e as Error).message}\n${HELP}`)
    return 2
  }
  if (parsed.values.help) {
    console.log(HELP)
    return 0
  }
  const cfg = parseApiCfg(parsed.values)
  if (!cfg) return 2
  const issueId = parsed.positionals[0]
  if (!issueId) {
    console.error('error: <issue-id> is required')
    return 2
  }
  try {
    await resolveIssue(cfg, issueId, parsed.values['in-release'] as string | undefined)
    console.log(`${issueId} → resolved${parsed.values['in-release'] ? ` (in ${parsed.values['in-release']})` : ''}`)
    return 0
  } catch (e) {
    console.error(`issue resolve failed: ${(e as Error).message}`)
    return 1
  }
}

async function cmdIssueNote(argv: string[]): Promise<number> {
  let parsed
  try {
    parsed = parseArgs({
      allowPositionals: true,
      args: argv,
      options: { ...ISSUE_OPTS, body: { type: 'string' } },
    })
  } catch (e) {
    console.error(`error: ${(e as Error).message}\n${HELP}`)
    return 2
  }
  if (parsed.values.help) {
    console.log(HELP)
    return 0
  }
  const cfg = parseApiCfg(parsed.values)
  if (!cfg) return 2
  const issueId = parsed.positionals[0]
  const body = parsed.values.body as string | undefined
  if (!issueId || !body) {
    console.error('error: <issue-id> and --body are required')
    return 2
  }
  try {
    await noteIssue(cfg, issueId, body)
    console.log(`${issueId} ← note added`)
    return 0
  } catch (e) {
    console.error(`issue note failed: ${(e as Error).message}`)
    return 1
  }
}

async function cmdIssueBundle(argv: string[]): Promise<number> {
  let parsed
  try {
    parsed = parseArgs({ allowPositionals: true, args: argv, options: ISSUE_OPTS })
  } catch (e) {
    console.error(`error: ${(e as Error).message}\n${HELP}`)
    return 2
  }
  if (parsed.values.help) {
    console.log(HELP)
    return 0
  }
  const cfg = parseApiCfg(parsed.values)
  if (!cfg) return 2
  const issueId = parsed.positionals[0]
  if (!issueId) {
    console.error('error: <issue-id> is required')
    return 2
  }
  try {
    console.log(await fetchBundle(cfg, issueId))
    return 0
  } catch (e) {
    console.error(`issue bundle failed: ${(e as Error).message}`)
    return 1
  }
}

// ── mcp ───────────────────────────────────────────────────────────

async function cmdMcpServe(argv: string[]): Promise<number> {
  let parsed
  try {
    parsed = parseArgs({ args: argv, options: ISSUE_OPTS })
  } catch (e) {
    console.error(`error: ${(e as Error).message}\n${HELP}`)
    return 2
  }
  if (parsed.values.help) {
    console.log(HELP)
    return 0
  }
  const cfg = parseApiCfg(parsed.values)
  if (!cfg) return 2
  try {
    await runMcpServer({ apiUrl: cfg.apiUrl, token: cfg.token })
    return 0
  } catch (e) {
    console.error(`mcp serve failed: ${(e as Error).message}`)
    return 1
  }
}

// ── the release gate ──────────────────────────────────────────────

/** What to upload instead, per kind. The server sends the same
 *  guidance in the upload response; a check run days later has only
 *  the listing, so the advice lives on both sides. */
const UNREADABLE_HINT: Record<string, string> = {
  dsym:
    'Upload the binary inside the .dSYM bundle ' +
    '(Contents/Resources/DWARF/<name>), or point `sentori-cli upload dsym` ' +
    'at the .dSYM and let it find the slices.',
  proguard: 'Upload build/outputs/mapping/<variant>/mapping.txt.',
  sourcemap:
    'For React Native upload the composed map — ' +
    '`sentori-cli react-native upload --metro-map <m> --hermes-map <h>` — not the bundle.',
}

async function cmdArtifactsCheck(argv: string[]): Promise<number> {
  let parsed
  try {
    parsed = parseArgs({
      allowPositionals: true,
      args: argv,
      options: { ...UPLOAD_OPTS, expect: { type: 'string' } },
    })
  } catch (e) {
    console.error(`error: ${(e as Error).message}`)
    return 2
  }
  const cfg = parseCommon(parsed.values)
  if (!cfg) return 2

  let res
  try {
    res = await fetchArtifacts(cfg)
  } catch (e) {
    console.error(`artifacts check failed: ${(e as Error).message}`)
    return 1
  }

  if (!res.known) {
    console.error(
      `Sentori has never seen the release "${cfg.release}".\n` +
        `  Either nothing was ever uploaded for it, or --release does not ` +
        `match the string your SDK passes to init({ release }).`,
    )
  }
  // An artifact the server could not parse is worse than a missing
  // one: it looks like coverage. Name it before the counts, because
  // the count already excludes it and the difference is otherwise
  // invisible.
  for (const a of res.artifacts.filter((x) => x.usable === false)) {
    console.error(
      `unreadable ${a.kind}: ${a.name}\n` +
        `  Stored, but the server cannot parse it, so it symbolicates nothing.\n` +
        `  ${UNREADABLE_HINT[a.kind] ?? 'Check that this is the file the kind expects.'}`,
    )
  }
  for (const [kind, n] of Object.entries(res.kinds)) {
    // Only the ones counted: printing a debug id beside a count of
    // zero reads as "no slices, here is a slice".
    const slices = res.artifacts.filter((a) => a.kind === kind && a.usable !== false)
    const ids = slices
      .map((a) => a.debugId)
      .filter((d): d is string => d !== null)
      .join(', ')
    console.log(`  ${kind.padEnd(10)} ${String(n).padStart(3)}${ids ? `  ${ids}` : ''}`)
  }

  const expect =
    typeof parsed.values.expect === 'string'
      ? parsed.values.expect
          .split(',')
          .map((k) => k.trim())
          .filter(Boolean)
      : []
  if (expect.length === 0) return 0

  const absent = expect.filter((k) => (res.kinds[k] ?? 0) === 0)
  if (absent.length === 0) {
    console.log(`ok — ${cfg.release} has ${expect.join(', ')}`)
    return 0
  }
  console.error(
    `missing for ${cfg.release}: ${absent.join(', ')}\n` +
      `  Stacks needing these stay unreadable for this release. Upload ` +
      `them and this passes — the upload re-reads the crashes already ` +
      `stored for this release (server >= 2.12.0), so the gap is not ` +
      `permanent.`,
  )
  return 1
}

async function main(argv: string[]): Promise<number> {
  if (argv.length === 0 || argv[0] === '-h' || argv[0] === '--help') {
    console.log(HELP)
    return 0
  }
  const [a, b, ...rest] = argv
  if (a === 'upload' && b === 'sourcemap') return cmdUploadSourcemap(rest)
  if (a === 'upload' && b === 'dsym') return cmdUploadDsym(rest)
  if (a === 'upload' && b === 'mapping') return cmdUploadMapping(rest)
  if (a === 'upload' && b === 'srcbundle') return cmdUploadSrcbundle(rest)
  if (a === 'react-native' && b === 'upload') return cmdReactNativeUpload(rest)
  if (a === 'artifacts' && b === 'check') return cmdArtifactsCheck(rest)
  if (a === 'probes' && b === 'sync') return cmdProbesSync(rest)
  if (a === 'mcp' && b === 'serve') return cmdMcpServe(rest)
  if (a === 'issue' && b === 'list') return cmdIssueList(rest)
  if (a === 'issue' && b === 'resolve') return cmdIssueResolve(rest)
  if (a === 'issue' && b === 'note') return cmdIssueNote(rest)
  if (a === 'issue' && b === 'bundle') return cmdIssueBundle(rest)
  if (a === 'push' && b === 'send') return cmdPushSend(rest)
  if (a === 'push' && b === 'receipt') return cmdPushReceipt(rest)
  if (a === 'push' && b === 'creds') {
    const [c, ...rest2] = rest
    if (c === 'list') return cmdPushCredsList(rest2)
    if (c === 'set') return cmdPushCredsSet(rest2)
    if (c === 'delete') return cmdPushCredsDelete(rest2)
  }
  console.error(`unknown command: ${[a, b].filter(Boolean).join(' ') || '(none)'}\n`)
  console.error(HELP)
  return 2
}

// ── push commands (v2.12) ─────────────────────────────────────────

async function cmdPushSend(argv: string[]): Promise<number> {
  const parsed = parseArgs({
    args: argv,
    options: {
      'api-url': { type: 'string' },
      body: { type: 'string' },
      data: { type: 'string' },
      'idempotency-key': { type: 'string' },
      'ingest-url': { type: 'string' },
      priority: { type: 'string' },
      project: { type: 'string' },
      title: { type: 'string' },
      to: { type: 'string' },
      token: { type: 'string' },
      ttl: { type: 'string' },
    },
    strict: true,
  })
  const cfg = parseAdminCfg(parsed.values)
  if (!cfg) return 2
  const to = parsed.values.to as string | undefined
  if (!to) {
    console.error('error: --to <ipt_handle> is required')
    return 2
  }
  try {
    const data = parsed.values.data ? (parseJsonArg(parsed.values.data as string, '--data') as Record<string, unknown>) : undefined
    const priority = parsed.values.priority as 'high' | 'normal' | undefined
    const ticket = await pushSend(cfg, {
      to,
      title: parsed.values.title as string | undefined,
      body: parsed.values.body as string | undefined,
      data,
      priority,
      ttl: parsed.values.ttl ? Number(parsed.values.ttl) : undefined,
      idempotencyKey: parsed.values['idempotency-key'] as string | undefined,
    })
    console.log(`${ticket.id} ${ticket.status}`)
    return 0
  } catch (e) {
    console.error(`push send failed: ${(e as Error).message}`)
    return 1
  }
}

async function cmdPushReceipt(argv: string[]): Promise<number> {
  const parsed = parseArgs({
    args: argv,
    options: {
      'api-url': { type: 'string' },
      'ingest-url': { type: 'string' },
      project: { type: 'string' },
      token: { type: 'string' },
    },
    allowPositionals: true,
    strict: true,
  })
  const sendId = parsed.positionals[0]
  if (!sendId) {
    console.error('error: <send-id> positional is required')
    return 2
  }
  const cfg = parseAdminCfg(parsed.values)
  if (!cfg) return 2
  try {
    const r = await pushReceipt(cfg, sendId)
    console.log(`${r.ticket.id} ${r.ticket.status}${r.ticket.providerOutcome ? ` (${r.ticket.providerOutcome})` : ''}${r.ticket.error ? ` — ${r.ticket.error}` : ''}`)
    return 0
  } catch (e) {
    console.error(`push receipt failed: ${(e as Error).message}`)
    return 1
  }
}

async function cmdPushCredsList(argv: string[]): Promise<number> {
  const parsed = parseArgs({
    args: argv,
    options: {
      'api-url': { type: 'string' },
      'ingest-url': { type: 'string' },
      project: { type: 'string' },
      token: { type: 'string' },
    },
    strict: true,
  })
  const cfg = parseAdminCfg(parsed.values)
  if (!cfg) return 2
  try {
    const rows = await pushCredsList(cfg)
    if (rows.length === 0) {
      console.log('(no providers configured)')
      return 0
    }
    for (const r of rows) {
      console.log(`${r.provider}\t${r.updatedAt}\t${JSON.stringify(r.config)}`)
    }
    return 0
  } catch (e) {
    console.error(`push creds list failed: ${(e as Error).message}`)
    return 1
  }
}

async function cmdPushCredsSet(argv: string[]): Promise<number> {
  const parsed = parseArgs({
    args: argv,
    options: {
      'api-url': { type: 'string' },
      config: { type: 'string' },
      'ingest-url': { type: 'string' },
      project: { type: 'string' },
      secret: { type: 'string' },
      token: { type: 'string' },
    },
    allowPositionals: true,
    strict: true,
  })
  const provider = parsed.positionals[0]
  if (!provider) {
    console.error('error: <provider> positional (apns/fcm/webpush/hcm/mipush) is required')
    return 2
  }
  const cfg = parseAdminCfg(parsed.values)
  if (!cfg) return 2
  if (!parsed.values.config || !parsed.values.secret) {
    console.error('error: --config @file.json and --secret @file.json are both required')
    return 2
  }
  try {
    const config = parseJsonArg(parsed.values.config as string, '--config')
    const secret = parseJsonArg(parsed.values.secret as string, '--secret')
    await pushCredsSet(cfg, provider, config, secret)
    console.log(`${provider} ✓ saved`)
    return 0
  } catch (e) {
    console.error(`push creds set failed: ${(e as Error).message}`)
    return 1
  }
}

async function cmdPushCredsDelete(argv: string[]): Promise<number> {
  const parsed = parseArgs({
    args: argv,
    options: {
      'api-url': { type: 'string' },
      'ingest-url': { type: 'string' },
      project: { type: 'string' },
      token: { type: 'string' },
    },
    allowPositionals: true,
    strict: true,
  })
  const provider = parsed.positionals[0]
  if (!provider) {
    console.error('error: <provider> positional is required')
    return 2
  }
  const cfg = parseAdminCfg(parsed.values)
  if (!cfg) return 2
  try {
    await pushCredsDelete(cfg, provider)
    console.log(`${provider} ✓ deleted`)
    return 0
  } catch (e) {
    console.error(`push creds delete failed: ${(e as Error).message}`)
    return 1
  }
}


main(process.argv.slice(2)).then(
  (code) => process.exit(code),
  (e: unknown) => {
    console.error(`fatal: ${(e as Error).message}`)
    process.exit(1)
  },
)
