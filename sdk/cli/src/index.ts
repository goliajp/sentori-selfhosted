#!/usr/bin/env node
import { parseArgs } from 'node:util'

import { formatIssueLine, issueList, issuePatch } from './issue.js'
import { runMcpServer } from './mcp.js'
import { uploadDsym, uploadMapping } from './native-artifacts.js'
import {
  parseJsonArg,
  pushCredsDelete,
  pushCredsList,
  pushCredsSet,
  pushReceipt,
  pushSend,
} from './push.js'
import { reactNativeUpload } from './react-native.js'
import { uploadSourceBundle } from './source-bundle.js'
import { uploadSourcemaps } from './upload.js'

const HELP = `sentori-cli — Sentori command-line interface

Source-map upload:
  sentori-cli upload sourcemap [options] <path...>
      Upload one or more files or directories. A directory is scanned
      (one level) for *.map / *.js / *.jsbundle / *.bundle / *.hbc;
      a file given explicitly is uploaded as-is. Use this when you
      already have a composed sourcemap on disk:
        - web bundlers (point at the build dir),
        - iOS post-\`react-native-xcode.sh\` where the build phase has
          already composed packager + Hermes maps into the final
          \`$SOURCEMAP_FILE\` and deleted the intermediates.
      Composed-then-uploaded vs raw-then-server-composed yield
      identical symbolication — the server stores the same shape
      either way.

  sentori-cli react-native upload [options]
      Compose a Metro packager map + a Hermes map into one source map
      (uses react-native's \`scripts/compose-source-maps.js\`) and
      upload the result. Use this when you have both raw maps on
      disk — typical Android release path where the gradle
      \`bundleReleaseJsAndAssets\` task leaves both maps untouched.
      Requires --metro-map and --hermes-map. On iOS the build phase
      deletes the intermediates, so use \`upload sourcemap\` instead.

Native artifacts (project-scoped, need --project + admin token):
  sentori-cli upload dsym --project <uuid> [--release <r>] [--object-name <n>] [--debug-id <uuid> --arch <a>] <path>
      Upload iOS dSYM debug info. By default walks a Foo.dSYM bundle
      and uses "dwarfdump --uuid" to enumerate slices, uploading each.
      Pass --debug-id and --arch to upload a single slice without
      dwarfdump (useful in Linux CI where the toolchain isn't there).

  sentori-cli upload mapping --project <uuid> [--release <r>] [--debug-id <uuid>] <mapping.txt>
      Upload an R8 / ProGuard mapping (raw bytes). If the file starts
      with a "# pg_map_id:" line the server sniffs the debug-id from
      it; otherwise you can pass it explicitly.

  sentori-cli upload source-bundle --project <uuid> --release <r> --platform ios|android [--module <label>] <archive.tar.gz>
      Upload a pre-built tar.gz of your project's source so the
      dashboard can render inline source for native (Swift / Kotlin /
      Objective-C) frames the way it already does for JS via source
      maps. Build the archive yourself:
        tar -czf ios-source.tar.gz Sources/
      Pass --module to upload multiple bundles per (release, platform)
      — e.g. \`--module main\`, \`--module watch-ext\`. Omitting --module
      reuses the v1.3 single-bundle slot (re-uploading replaces it).

CI triage:
  sentori-cli issue list --project <uuid> [--status active|silenced|resolved|closed] [--limit N] [--error-type <t>]
  sentori-cli issue resolve <issue-uuid> --project <uuid> [--in-release <r>]
  sentori-cli issue silence <issue-uuid> --project <uuid>

LLM agents (MCP):
  sentori-cli mcp serve --project <uuid> [--token <t>] [--api-url <url>]
      Run a stdio MCP server. Connect from Claude Code / any MCP
      client by pointing at \`sentori-cli mcp serve …\` as the command.
      Exposes sentori_issue_list / _get / _comment / _transition /
      _assign / _set_priority / _set_labels / _watch tools.

Options (upload commands):
  --release <r>     release identifier — MUST equal the value the SDK
                    reports via init({ release }). Required.
  --token <t>       Sentori token (or set $SENTORI_TOKEN).
  --api-url <url>   Sentori API base (default https://sentori.golia.jp,
                    or $SENTORI_API_URL). For a self-hosted instance, your
                    host. (Accepts --ingest-url as an alias.)
  --dry-run         describe what would be uploaded; don't upload.
  -h, --help        show this help.

Options (react-native upload):
  --metro-map <p>   the *.packager.map Metro emits (--sourcemap-output).
  --hermes-map <p>  the *.hbc.map the Hermes compiler emits.
  --bundle <p>      optional: also upload the bundle (.jsbundle / .bundle).

Options (issue commands):
  --project <uuid>  project id (or set $SENTORI_PROJECT_ID).
  --token <t>       admin token, sk_… prefix (or $SENTORI_ADMIN_TOKEN /
                    $SENTORI_TOKEN). The ingest st_pk_ token may also work
                    on a self-hosted instance.
  --api-url <url>   Sentori API base (same as above).
  --in-release <r>  (resolve only) mark this release as where the fix
                    landed; the regression detector flips the issue back
                    to "regressed" if a matching event lands later.

Hermes release build, by hand:

  Android (raw maps still on disk after \`./gradlew bundleRelease\`):
    npx @goliapkg/sentori-cli react-native upload \\
      --release "<app>@<version>+<build>" --token "$SENTORI_TOKEN" \\
      --metro-map  android/app/build/intermediates/sourcemaps/react/release/index.android.bundle.packager.map \\
      --hermes-map android/app/build/intermediates/sourcemaps/react/release/index.android.bundle.compiler.map \\
      --bundle     android/app/build/generated/assets/react/release/index.android.bundle

  iOS (already-composed map after \`xcodebuild archive\`; the build
       phase deletes intermediates so you only have the composed map):
    npx @goliapkg/sentori-cli upload sourcemap \\
      --release "<app>@<version>+<build>" --token "$SENTORI_TOKEN" \\
      "$BUILT_PRODUCTS_DIR/main.jsbundle.map" \\
      "$BUILT_PRODUCTS_DIR/main.jsbundle"
`

type Common = { apiUrl: string; dryRun: boolean; release: string; token: string }

/** Parse the shared options, or print an error + return null. */
function parseCommon(values: Record<string, unknown>): Common | null {
  const release = typeof values.release === 'string' ? values.release : undefined
  if (!release) {
    console.error('error: --release is required (must match the SDK’s init({ release }))')
    return null
  }
  const dryRun = values['dry-run'] === true
  const token =
    (typeof values.token === 'string' ? values.token : undefined) ?? process.env.SENTORI_TOKEN
  if (!token && !dryRun) {
    console.error('error: --token (or $SENTORI_TOKEN) is required')
    return null
  }
  const apiUrl =
    (typeof values['api-url'] === 'string' ? values['api-url'] : undefined) ??
    (typeof values['ingest-url'] === 'string' ? values['ingest-url'] : undefined) ??
    process.env.SENTORI_API_URL ??
    'https://sentori.golia.jp'
  return { apiUrl, dryRun, release, token: token ?? '' }
}

async function cmdUploadSourcemap(argv: string[]): Promise<number> {
  let parsed
  try {
    parsed = parseArgs({
      allowPositionals: true,
      args: argv,
      options: {
        'api-url': { type: 'string' },
        'dry-run': { type: 'boolean' },
        help: { short: 'h', type: 'boolean' },
        'ingest-url': { type: 'string' },
        release: { type: 'string' },
        token: { type: 'string' },
      },
    })
  } catch (e) {
    console.error(`error: ${(e as Error).message}\n`)
    console.error(HELP)
    return 2
  }
  if (parsed.values.help) {
    console.log(HELP)
    return 0
  }
  const c = parseCommon(parsed.values)
  if (!c) return 2
  if (parsed.positionals.length === 0) {
    console.error('error: at least one path (file or directory) is required')
    return 2
  }
  try {
    const result = await uploadSourcemaps({
      apiUrl: c.apiUrl,
      dryRun: c.dryRun,
      paths: parsed.positionals,
      release: c.release,
      token: c.token,
    })
    reportUpload(result, c)
    return 0
  } catch (e) {
    console.error(`upload failed: ${(e as Error).message}`)
    return 1
  }
}

async function cmdReactNativeUpload(argv: string[]): Promise<number> {
  let parsed
  try {
    parsed = parseArgs({
      args: argv,
      options: {
        'api-url': { type: 'string' },
        bundle: { type: 'string' },
        'dry-run': { type: 'boolean' },
        help: { short: 'h', type: 'boolean' },
        'hermes-map': { type: 'string' },
        'ingest-url': { type: 'string' },
        'metro-map': { type: 'string' },
        release: { type: 'string' },
        token: { type: 'string' },
      },
    })
  } catch (e) {
    console.error(`error: ${(e as Error).message}\n`)
    console.error(HELP)
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
      dryRun: c.dryRun,
      hermesMap,
      metroMap,
      release: c.release,
      token: c.token,
    })
    reportUpload(result, c)
    return 0
  } catch (e) {
    console.error(`react-native upload failed: ${(e as Error).message}`)
    return 1
  }
}

function reportUpload(
  result: { files: string[]; uploaded?: number },
  c: Common,
): void {
  if (c.dryRun) {
    console.log(
      `would upload ${result.files.length} file(s) to ${c.apiUrl.replace(/\/+$/, '')}/admin/api/releases/${encodeURIComponent(c.release)}/sourcemaps:`,
    )
    for (const f of result.files) console.log(`  ${f}`)
  } else {
    console.log(
      `uploaded ${result.uploaded ?? result.files.length} file(s) for release "${c.release}" — minified stacks on this release will now resolve to source.`,
    )
  }
}

// ── issue commands ────────────────────────────────────────────────

type AdminCfg = { apiUrl: string; projectId: string; token: string }

function parseAdminCfg(values: Record<string, unknown>): AdminCfg | null {
  const projectId =
    (typeof values.project === 'string' ? values.project : undefined) ??
    process.env.SENTORI_PROJECT_ID
  if (!projectId) {
    console.error('error: --project <uuid> (or $SENTORI_PROJECT_ID) is required')
    return null
  }
  const token =
    (typeof values.token === 'string' ? values.token : undefined) ??
    process.env.SENTORI_ADMIN_TOKEN ??
    process.env.SENTORI_TOKEN
  if (!token) {
    console.error(
      'error: --token (or $SENTORI_ADMIN_TOKEN / $SENTORI_TOKEN) is required for issue commands',
    )
    return null
  }
  const apiUrl =
    (typeof values['api-url'] === 'string' ? values['api-url'] : undefined) ??
    (typeof values['ingest-url'] === 'string' ? values['ingest-url'] : undefined) ??
    process.env.SENTORI_API_URL ??
    'https://sentori.golia.jp'
  return { apiUrl, projectId, token }
}

async function cmdIssueList(argv: string[]): Promise<number> {
  let parsed
  try {
    parsed = parseArgs({
      args: argv,
      options: {
        'api-url': { type: 'string' },
        'error-type': { type: 'string' },
        help: { short: 'h', type: 'boolean' },
        'ingest-url': { type: 'string' },
        limit: { type: 'string' },
        project: { type: 'string' },
        status: { type: 'string' },
        token: { type: 'string' },
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
  const cfg = parseAdminCfg(parsed.values)
  if (!cfg) return 2
  const status = parsed.values.status
  if (status && !['active', 'closed', 'resolved', 'silenced'].includes(status)) {
    console.error(`error: --status must be one of: active, silenced, resolved, closed`)
    return 2
  }
  const limitStr = parsed.values.limit
  const limit = limitStr ? Number.parseInt(limitStr, 10) : undefined
  try {
    const rows = await issueList({
      config: cfg,
      errorType: parsed.values['error-type'],
      limit,
      status: status as 'active' | 'closed' | 'resolved' | 'silenced' | undefined,
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

async function cmdIssuePatch(
  argv: string[],
  body: { resolvedInRelease?: string; status: 'active' | 'closed' | 'resolved' | 'silenced' },
  verb: 'closed' | 'resolved' | 'silenced',
): Promise<number> {
  let parsed
  try {
    parsed = parseArgs({
      allowPositionals: true,
      args: argv,
      options: {
        'api-url': { type: 'string' },
        help: { short: 'h', type: 'boolean' },
        'in-release': { type: 'string' },
        'ingest-url': { type: 'string' },
        project: { type: 'string' },
        token: { type: 'string' },
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
  const cfg = parseAdminCfg(parsed.values)
  if (!cfg) return 2
  const issueId = parsed.positionals[0]
  if (!issueId) {
    console.error('error: <issue-uuid> is required')
    return 2
  }
  if (verb === 'resolved' && typeof parsed.values['in-release'] === 'string') {
    body.resolvedInRelease = parsed.values['in-release']
  }
  try {
    const updated = await issuePatch(cfg, issueId, body)
    console.log(
      `${issueId} → ${verb}${body.resolvedInRelease ? ` (in ${body.resolvedInRelease})` : ''}: ${updated.errorType}`,
    )
    return 0
  } catch (e) {
    console.error(`issue ${verb} failed: ${(e as Error).message}`)
    return 1
  }
}

// ── native artifact upload ────────────────────────────────────────

async function cmdUploadDsym(argv: string[]): Promise<number> {
  let parsed
  try {
    parsed = parseArgs({
      allowPositionals: true,
      args: argv,
      options: {
        'api-url': { type: 'string' },
        arch: { type: 'string' },
        'debug-id': { type: 'string' },
        help: { short: 'h', type: 'boolean' },
        'ingest-url': { type: 'string' },
        'object-name': { type: 'string' },
        project: { type: 'string' },
        release: { type: 'string' },
        token: { type: 'string' },
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
  const cfg = parseAdminCfg(parsed.values)
  if (!cfg) return 2
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
      apiUrl: cfg.apiUrl,
      arch: typeof arch === 'string' ? arch : undefined,
      debugId: typeof debugId === 'string' ? debugId : undefined,
      objectName: typeof parsed.values['object-name'] === 'string' ? parsed.values['object-name'] : undefined,
      path,
      projectId: cfg.projectId,
      release: typeof parsed.values.release === 'string' ? parsed.values.release : undefined,
      token: cfg.token,
    })
    console.log(`uploaded ${r.slices.length} dSYM slice(s):`)
    for (const s of r.slices) console.log(`  ${s.debugId}  (${s.arch})`)
    return 0
  } catch (e) {
    console.error(`dsym upload failed: ${(e as Error).message}`)
    return 1
  }
}

async function cmdUploadMapping(argv: string[]): Promise<number> {
  let parsed
  try {
    parsed = parseArgs({
      allowPositionals: true,
      args: argv,
      options: {
        'api-url': { type: 'string' },
        'debug-id': { type: 'string' },
        help: { short: 'h', type: 'boolean' },
        'ingest-url': { type: 'string' },
        project: { type: 'string' },
        release: { type: 'string' },
        token: { type: 'string' },
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
  const cfg = parseAdminCfg(parsed.values)
  if (!cfg) return 2
  const path = parsed.positionals[0]
  if (!path) {
    console.error('error: a path to mapping.txt is required')
    return 2
  }
  try {
    await uploadMapping({
      apiUrl: cfg.apiUrl,
      debugId: typeof parsed.values['debug-id'] === 'string' ? parsed.values['debug-id'] : undefined,
      path,
      projectId: cfg.projectId,
      release: typeof parsed.values.release === 'string' ? parsed.values.release : undefined,
      token: cfg.token,
    })
    console.log(`uploaded mapping for project ${cfg.projectId}${parsed.values.release ? ` / ${parsed.values.release}` : ''}`)
    return 0
  } catch (e) {
    console.error(`mapping upload failed: ${(e as Error).message}`)
    return 1
  }
}

async function cmdUploadSourceBundle(argv: string[]): Promise<number> {
  let parsed
  try {
    parsed = parseArgs({
      allowPositionals: true,
      args: argv,
      options: {
        'api-url': { type: 'string' },
        help: { short: 'h', type: 'boolean' },
        'ingest-url': { type: 'string' },
        // v1.4 W26 — optional module label so polyrepo apps can
        // upload multiple bundles per (release, platform) without
        // clobbering each other (main vs watch-ext vs share-ext…).
        module: { type: 'string' },
        platform: { type: 'string' },
        project: { type: 'string' },
        release: { type: 'string' },
        token: { type: 'string' },
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
  const cfg = parseAdminCfg(parsed.values)
  if (!cfg) return 2
  const path = parsed.positionals[0]
  if (!path) {
    console.error('error: a path to a tar.gz archive is required')
    return 2
  }
  const platform = parsed.values.platform
  if (platform !== 'ios' && platform !== 'android') {
    console.error('error: --platform must be ios or android')
    return 2
  }
  const release = typeof parsed.values.release === 'string' ? parsed.values.release : undefined
  if (!release) {
    console.error('error: --release is required for source-bundle uploads')
    return 2
  }
  try {
    const r = await uploadSourceBundle({
      apiUrl: cfg.apiUrl,
      module: typeof parsed.values.module === 'string' ? parsed.values.module : undefined,
      path,
      platform,
      projectId: cfg.projectId,
      release,
      token: cfg.token,
    })
    console.log(`uploaded ${r.kind} (${r.sizeBytes} bytes, sha256:${r.contentHash.slice(0, 12)}…)`)
    return 0
  } catch (e) {
    console.error(`source-bundle upload failed: ${(e as Error).message}`)
    return 1
  }
}

async function cmdMcpServe(argv: string[]): Promise<number> {
  let parsed
  try {
    parsed = parseArgs({
      args: argv,
      options: {
        'api-url': { type: 'string' },
        help: { short: 'h', type: 'boolean' },
        'ingest-url': { type: 'string' },
        project: { type: 'string' },
        token: { type: 'string' },
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
  const cfg = parseAdminCfg(parsed.values)
  if (!cfg) return 2
  try {
    await runMcpServer({ apiUrl: cfg.apiUrl, projectId: cfg.projectId, token: cfg.token })
    return 0
  } catch (e) {
    console.error(`mcp serve failed: ${(e as Error).message}`)
    return 1
  }
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
  if (a === 'upload' && b === 'source-bundle') return cmdUploadSourceBundle(rest)
  if (a === 'mcp' && b === 'serve') return cmdMcpServe(rest)
  if (a === 'react-native' && b === 'upload') return cmdReactNativeUpload(rest)
  if (a === 'issue' && b === 'list') return cmdIssueList(rest)
  if (a === 'issue' && b === 'resolve') return cmdIssuePatch(rest, { status: 'resolved' }, 'resolved')
  if (a === 'issue' && b === 'silence') return cmdIssuePatch(rest, { status: 'silenced' }, 'silenced')
  if (a === 'issue' && b === 'close') return cmdIssuePatch(rest, { status: 'closed' }, 'closed')
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
