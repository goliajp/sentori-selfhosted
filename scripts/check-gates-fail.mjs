// A gate that cannot fail is not a gate.
//
// Every checker here was green the day it was written, and green ever
// since. That is also what a checker looks like after the code it reads
// moves out from under it: `check-wire-case` reported "all camelCase"
// for the life of `/v1/releases/{release}/artifacts`, which answered
// content_hash and size_bytes, because it read one directory and the
// route lived one level up.
//
// So each entry below reintroduces the defect its checker exists for and
// requires the checker to say so. The mutation is applied to a copy of
// the tracked tree, never to the working tree.
//
// Injections happen where the rule is, not near it. Three of the first
// attempts at this table passed against a live gate: a snake_case key in
// the admin API, which `check-wire-case` excludes on purpose; a removed
// `use` line rather than the guard call itself; and an error body in a
// handler that carries its own status, which is what the rule asks for.
// An entry that does not go red means the probe is wrong at least as
// often as the gate.
import { execFileSync, spawnSync } from 'node:child_process';
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';

const ROOT = new URL('..', import.meta.url).pathname;

const PROBES = [
  {
    gate: 'check-wire-case.mjs',
    file: 'self-hosted/server/src/handlers/sdk/events.rs',
    find: 'json!({',
    replace: 'json!({\n        "injected_snake_key": 1,',
    why: 'a snake_case key on the /v1 wire',
  },
  {
    gate: 'check-text-ordering.mjs',
    file: 'self-hosted/server/src/handlers/notify_admin.rs',
    find: 'ORDER BY p.name COLLATE \\"C\\"',
    replace: 'ORDER BY p.name',
    why: 'an order the operator\'s image decides',
  },
  {
    gate: 'check-metric-names.mjs',
    file: 'ops/prometheus-alerts.yml',
    find: 'groups:',
    replace:
      'groups:\n  - name: injected\n    rules:\n      - alert: Injected\n' +
      '        expr: sentori_does_not_exist > 0',
    why: 'an alert on a metric nothing emits',
  },
  {
    gate: 'check-sql-tables-exist.mjs',
    file: 'self-hosted/server/src/resymbolicate.rs',
    find: 'FROM releases ORDER BY created_at DESC',
    replace: 'FROM releases_nonexistent ORDER BY created_at DESC',
    why: 'a query naming a table the migrations never create',
  },
  {
    gate: 'check-attachment-scoping.mjs',
    file: 'self-hosted/server/src/handlers/attachments.rs',
    find: '"SELECT project_id, media_type, blob_hash FROM event_attachments',
    replace: '"SELECT media_type, blob_hash FROM event_attachments',
    why: 'a read of another project\'s attachment',
  },
  {
    gate: 'check-admin-authorisation.mjs',
    file: 'self-hosted/server/src/handlers/admin/releases.rs',
    find: '    ensure_project_access(&state, &ctx, project_id).await?;',
    replace: '    // guard removed by the probe',
    why: 'a project-scoped admin endpoint that authorises nobody',
  },
  {
    gate: 'check-timezone-pinned.mjs',
    file: 'self-hosted/cli/src/db.rs',
    find: 'const TIME_ZONE: &str = "UTC";',
    replace: 'const TIME_ZONE: &str = "Asia/Tokyo";',
    why: 'two binaries meaning different things by now()',
  },
  {
    gate: 'check-wire-contracts.mjs',
    file: 'self-hosted/server/src/handlers/sdk/events.rs',
    find: '"javascript", "ios", "android", "web", "weapp"',
    replace: '"javascript", "ios", "android", "weapp"',
    why: 'a runtime the SDK names and the server files under unknown',
  },
  {
    gate: 'check-ios-packaging.mjs',
    file: 'sdk/native/ios/Sources/Sentori/PrivacyInfo.xcprivacy',
    find: 'NSPrivacyAccessedAPICategoryUserDefaults',
    replace: 'NSPrivacyAccessedAPICategoryUserDefaultsTypo',
    why: 'a required-reason API the manifest does not declare',
  },
  {
    gate: 'check-doc-versions.mjs',
    file: 'docs/sdk-kotlin.md',
    find: 'jp.golia.sentori:sentori:',
    replace: 'jp.golia.sentori:sentori:0.0.1-',
    why: 'an install line that installs a version we do not ship',
  },
  {
    gate: 'gen-replay-vectors.mjs',
    // The compiled module, not the source: the generator imports
    // `lib/`, so a mutation of the `.ts` would leave the checker
    // reading the same bytes and passing.
    file: 'sdk/core/lib/replay-ring.js',
    find: 'const DELTA_TO_KEYFRAME_RATIO = 0.4',
    replace: 'const DELTA_TO_KEYFRAME_RATIO = 0.9',
    why: 'a replay rule the native ports are no longer asserting',
  },
  {
    gate: 'check-doc-commands.mjs',
    file: 'docs/sdk-swift.md',
    find: '--token "$SENTORI_TOKEN"',
    replace: '--token "$SENTORI_API_TOKEN"',
    why: 'a documented token variable the CLI does not read',
  },
  {
    gate: 'check-orphan-ts.mjs',
    file: 'sdk/react-native/src/index.ts',
    // `mask` until 2026-09-30, when this stopped orphaning anything:
    // `replay.ts` and `replay-screens.ts` both import it now, so
    // dropping the re-export left the module perfectly reachable and
    // the gate rightly said nothing. The probe had been passing on a
    // non-zero exit for an unrelated reason; the baseline check added
    // to this file is what exposed it.
    //
    // `error-boundary` is reachable from the index and nowhere else,
    // which is what an orphan probe needs.
    find: "export { ErrorBoundary } from './error-boundary';",
    replace: '',
    why: 'a TypeScript module that ships in no bundle',
  },
  {
    // The rigs that crash a real app are the gates nobody can retest
    // by hand, so a trigger list that forgets one is the quietest way
    // to lose them.
    gate: 'check-workflow-script-paths.mjs',
    file: '.github/workflows/mobile-e2e.yml',
    find: "      - 'scripts/ios-crash-loop.sh'",
    replace: '',
    why: 'a gate script no workflow is triggered by',
  },
  {
    gate: 'check-surface-gates.mjs',
    file: '.github/workflows/build.yml',
    find: '          - sdk/weapp\n',
    replace: '',
    why: 'a package under sdk/ that no job builds',
  },
  {
    gate: 'check-sdk-doc-options.mjs',
    file: 'sdk/core/src/types.ts',
    // At the top level of `InitConfig`. The first version of this
    // probe added the field inside `detect`, which that checker reads
    // past — it takes top-level fields only — so it stayed green and
    // said nothing about itself.
    find: '  /** B-type replay rolling buffer, seconds. 0 disables. */',
    replace: '  undocumentedOption?: string\n  /** B-type replay rolling buffer, seconds. 0 disables. */',
    why: 'a public option the SDK reference does not mention',
  },
  {
    gate: 'check-md-fences.mjs',
    file: 'sdk/web/README.md',
    find: '```bash\nbun add @goliapkg/sentori-web\n```',
    replace: '```bash\nbun add @goliapkg/sentori-web',
    why: 'a code fence that never closes',
  },
  {
    // A second launcher is a second set of flags nobody compares
    // until one of them is flaky on a machine nobody can log into.
    gate: 'check-single-chrome-launcher.mjs',
    file: 'scripts/lib/headless-chrome.mjs',
    find: "      '--no-first-run',",
    replace: '',
    why: 'a launcher missing a flag that is there for a real failure',
  },
  {
    // The fixture is generated from the kernel, so a kernel rule that
    // Swift and Kotlin have not been told about shows up here rather
    // than as two platforms counting losses differently in
    // production.
    gate: 'gen-transport-vectors.mjs',
    // The built lib, not the source: the generator drives the compiled
    // kernel, so that is what a stale fixture would disagree with.
    file: 'sdk/core/lib/transport.js',
    find: 'const MAX_QUEUED = 500',
    replace: 'const MAX_QUEUED = 400',
    why: 'a kernel rule the native transports have not followed',
  },
  {
    // Adds a dead option rather than removing a read. Every switch in
    // `detect` is read in two places — once to resolve the config and
    // once where it acts — so deleting one line leaves the other, and
    // a probe that cannot make the gate red proves nothing about
    // either.
    gate: 'check-dead-options.mjs',
    file: 'sdk/core/src/types.ts',
    find: '    uiThreadHang?: boolean',
    replace: '    uiThreadHang?: boolean\n    neverReadByAnything?: boolean',
    why: 'a public option nothing reads',
  },
  {
    gate: 'check-compose-healthchecks.mjs',
    file: 'self-hosted/docker/docker-compose.yml',
    find: 'test: ["CMD", "pg_isready", "-U", "sentori"]',
    replace: 'test: ["CMD-SHELL", "pg_isready -U sentori"]',
    why: 'a healthcheck an image without a shell cannot answer',
  },
  {
    gate: 'check-time-has-absolute.mjs',
    file: 'webapp/src/pages/Instruments.tsx',
    // The waiver, not the call. Deleting the call would only prove the
    // checker counts occurrences; deleting the reason it is allowed is
    // the defect — a relative time with nothing behind it.
    find: '                            // bare-relative: interpolated into a sentence, no element to hold a title\n          since: formatRelative(p.registeredAt),',
    replace: '          since: formatRelative(p.registeredAt),',
    why: 'a bare relative time with no absolute behind it and no reason given',
  },
  {
    gate: 'check-bun-version.mjs',
    file: '.bun-version',
    find: '1.4.2',
    replace: '1.3.13',
    why: 'a pinned bun that is not the bun writing the lockfiles',
  },
  {
    gate: 'check-doc-links.mjs',
    file: 'docs/README.md',
    find: '[Error reference](errors.md)',
    replace: '[Error reference](error-codes.md)',
    why: 'an index claiming a page that does not exist',
  },
  {
    gate: 'check-sql-inserts.mjs',
    file: 'self-hosted/server/src/handlers/admin/test_push.rs',
    find: "VALUES ($1, $2, $3, $4, $5, 'queued') RETURNING id",
    replace: "VALUES ($1, $2, $3, $4, $5, $6, 'queued') RETURNING id",
    why: 'an INSERT with more values than columns, which Postgres refuses at prepare time',
  },
  {
    // actionlint is a separate binary. Where it is missing the gate
    // soft-skips by design, so the probe is skipped too and reported
    // as unverified rather than quietly passing.
    gate: 'check-workflows.sh',
    requires: 'actionlint',
    file: '.github/workflows/build.yml',
    find: 'jobs:\n  changes:',
    replace: 'jobs:\n  changes:\n    runs-on: ${{ }}',
    why: 'a workflow GitHub cannot parse',
  },
  {
    gate: 'check-mirror.mjs',
    file: '.github/workflows/v0.2-oss-mirror.yml',
    // A page the index links and the mirror would leave behind. The
    // allowlist is per-file, so writing a page and publishing it were
    // two actions until this caught them apart.
    find: "            --include='/docs/dashboard.md' \\\n",
    replace: '',
    why: 'a docs page the public repository would not have',
  },
  {
    gate: 'check-orphan-modules.sh',
    file: 'self-hosted/server/src/main.rs',
    find: 'mod archive_worker;\n',
    replace: '',
    why: 'a module in the tree that nothing compiles',
  },
  {
    gate: 'check-docs-api-truth.mjs',
    file: 'docs/protocol.md',
    find: 'sentori.context(',
    replace: 'sentori.startSpan(',
    why: 'a doc teaching a verb the SDK does not export',
  },
  {
    gate: 'check-env-vars-real.mjs',
    file: 'docs/troubleshooting.md',
    find: 'SENTORI_RATELIMIT_PER_TOKEN_RPS',
    replace: 'SENTORI_RATE_LIMIT_PER_MIN',
    why: 'an env var the docs name and nothing reads',
  },
  {
    gate: 'check-doc-imports.mjs',
    file: 'docs/sdk-kotlin.md',
    find: 'import com.sentori.SentoriConfig',
    replace: 'import jp.golia.sentori.SentoriConfig',
    why: 'an import built from the Gradle coordinate rather than the package',
  },
  {
    gate: 'check-push-snippets.mjs',
    file: 'webapp/src/lib/push-snippets.ts',
    find: "export const SEND_PATH = '/v1/push/sends';",
    replace: "export const SEND_PATH = '/v1/push/send';",
    why: 'snippets teaching a route the server does not register',
  },
  {
    gate: 'check-i18n.mjs',
    file: 'webapp/src/i18n/en.ts',
    find: "  'platform.unknown': 'Unknown (SDK newer than this server)',",
    replace:
      "  'platform.unknown': 'Unknown (SDK newer than this server)',\n" +
      "  'zz.neverReferenced': 'a key no screen asks for',",
    why: 'a message key no screen references, and no other locale has',
  },
  {
    gate: 'check-hardcoded-text.mjs',
    file: 'webapp/src/pages/Projects.tsx',
    find: '        {row.platform && (',
    replace: "        <span>A sentence nobody ever translated</span>\n        {row.platform && (",
    why: 'English prose on screen that never went through t()',
  },
  {
    gate: 'check-cjk-punctuation.mjs',
    file: 'webapp/src/i18n/ja.ts',
    // ja.ts, because the checker read only zh.ts for as long as it
    // existed while its own opening note counted the marks it had
    // found in ja.ts.
    find: "  'platform.unknown': '不明（SDK がサーバーより新しい）',",
    replace: "  'platform.unknown': '不明(SDK がサーバーより新しい)',",
    why: 'half-width brackets against Japanese UI copy',
  },
  {
    gate: 'check-no-raw-fetch.mjs',
    file: 'webapp/src/pages/Projects.tsx',
    find: '  const t = useT();\n  const h = row.health;',
    replace: "  const t = useT();\n  void fetch('/admin/api/projects');\n  const h = row.health;",
    why: 'a UI file calling fetch instead of going through the api client',
  },
  {
    gate: 'check-platform-label.mjs',
    file: 'webapp/src/pages/Projects.tsx',
    find: '            {platformLabel(row.platform, t)}',
    replace: '            {row.platform}',
    why: 'a wire platform value printed raw',
  },
  {
    gate: 'check-unreferenced.mjs',
    file: 'webapp/src/pages/IssueDetail.tsx',
    find: "import { UserChip } from '../components/identity';\n",
    replace: '',
    why: 'a module left behind that nothing imports',
  },
  {
    gate: 'check-select-appearance.mjs',
    file: 'webapp/src/pages/TriageView.tsx',
    find: "                className={`${SELECT_CLASS} mr-1 h-[22px] pl-1 pr-5 text-xs text-fg-muted`}\n",
    replace: '                className="mr-1 h-[22px] pl-1 pr-5 text-xs text-fg-muted"\n',
    why: 'a select that paints its own light-grey control over a dark form',
  },
  {
    gate: 'check-audit-columns.mjs',
    file: 'webapp/src/pages/Settings.tsx',
    find: '<span className="text-fg-muted">{r.targetType ?? \'—\'}</span>',
    replace: '<span className="text-fg-muted" />',
    why: 'an audit row that no longer says what kind of thing was acted on',
  },
  {
    gate: 'check-error-format.mjs',
    file: 'webapp/src/lib/useAsyncData.ts',
    find: 'return e instanceof ApiError ? `${e.status}: ${e.message}` : String(e);',
    replace: 'return e instanceof ApiError ? `: ` : String(e);',
    why: 'every API failure in the console rendering as a bare colon',
  },
  {
    gate: 'check-release-format.mjs',
    file: 'webapp/src/components/ui.tsx',
    find: '  return collides ? release : short;',
    replace: '  return short;',
    why: 'two different builds drawn as one release',
  },
  {
    gate: 'check-error-reason.mjs',
    file: 'webapp/src/pages/Instruments.tsx',
    find: '<ErrorBanner reason={error}>',
    replace: '<ErrorBanner>',
    why: 'a banner that drops what the server said and keeps only our sentence',
  },
  {
    gate: 'check-release-lights.mjs',
    file: 'webapp/src/pages/Releases.tsx',
    find: "    ['unused', t('releases.legendUnused')],\n",
    replace: '',
    why: 'a legend that leaves one of the dots unexplained',
  },
  {
    gate: 'check-rfc3339.sh',
    file: 'self-hosted/server/src/handlers/sdk/events.rs',
    find: '    #[serde(with = "time::serde::rfc3339")]\n    pub occurred_at: OffsetDateTime,',
    replace: '    pub occurred_at: OffsetDateTime,',
    why: 'a timestamp that goes out as a nine-element array and parses as NaN',
  },
  {
    gate: 'gen-openapi.mjs',
    file: 'self-hosted/server/src/handlers/mod.rs',
    // Under /v1, because the document covers the machine-facing
    // surface only — a route outside it is excluded on purpose, and
    // the first version of this probe added one there and proved
    // nothing.
    find: '.route("/v1/deploys", post(sdk::deploys::handle))',
    replace: '.route("/v1/deploys", post(sdk::deploys::handle))\n        .route("/v1/injected-probe", post(sdk::deploys::handle))',
    why: 'a route the published OpenAPI document does not mention',
  },
  {
    gate: 'gen-error-reference.mjs',
    file: 'self-hosted/server/src/handlers/sdk/events.rs',
    find: 'Json(json!({ "error": "ingest_failed" })),',
    replace: 'Json(json!({ "error": "injected_probe_code" })),',
    why: 'an error code the published reference does not list',
  },
  {
    gate: 'check-crash-free-format.mjs',
    deps: 'webapp/node_modules',
    file: 'webapp/src/lib/crash-free.ts',
    find: '  if (crashed <= 0) return pct.toFixed(digits);\n  const scale = 10 ** digits;\n  return (Math.floor(pct * scale) / scale).toFixed(digits);',
    replace: '  return pct.toFixed(digits);',
    why: 'a release with three crashes rendering as 100%',
  },
  {
    gate: 'check-timeline-labels.mjs',
    deps: 'webapp/node_modules',
    file: 'webapp/src/components/TimelineStrip.tsx',
    find: '  return ((-sec / spanS) * scale) * trackW >= EVENT_LABEL_PX;',
    replace: '  return true;',
    why: 'a tick label printed inside the space the event label reserves',
  },
  {
    gate: 'check-byte-format.mjs',
    deps: 'webapp/node_modules',
    file: 'webapp/src/components/ui.tsx',
    find: "  const units = ['KB', 'MB', 'GB', 'TB'];",
    replace: "  const units = ['KB'];",
    why: 'a size that never changes unit, so a 291 MB dSYM reads as six digits of KB',
  },
  {
    gate: 'check-peer-ranges.mjs',
    deps: 'sdk/react-native/node_modules',
    file: 'sdk/react-native/package.json',
    find: '"react-native": ">=0.86.0"',
    replace: '"react-native": ">=0.99.0"',
    why: 'a published peer range that excludes the version we build against',
  },
  {
    gate: 'check-highlight.mjs',
    deps: 'webapp/node_modules',
    file: 'webapp/src/lib/highlight.ts',
    // The registration, not `languageForPath` — the checker runs
    // every console snippet through `highlightBlock` and never asks
    // what language a file path is, so a probe there proved nothing.
    find: "  ['rust', rust],\n",
    replace: '',
    why: 'a snippet language with no grammar registered, so it renders uncoloured',
  },
  {
    gate: 'check-credential-recognition.mjs',
    deps: 'webapp/node_modules',
    file: 'webapp/src/lib/push-credentials.ts',
    find: "  if (body.startsWith('-----BEGIN')) {",
    replace: "  if (false) {",
    why: 'a PEM key the credentials form no longer recognises',
  },
  {
    gate: 'check-doc-reachable.mjs',
    file: 'docs/README.md',
    find: '- [Release versioning](recipes/release-versioning.md)\n',
    replace: '',
    why: 'a docs page with no way in from the index',
  },
  {
    gate: 'check-public-links.mjs',
    file: 'docs/troubleshooting.md',
    find: 'https://github.com/goliajp/sentori-selfhosted/issues',
    replace: 'https://github.com/goliajp/sentori/issues',
    why: 'the one instruction for getting help pointing at a private repository',
  },
  {
    gate: 'check-ui-sentence-final.mjs',
    file: 'webapp/src/i18n/zh.ts',
    find: "  'inbox.loadFailed': '收件箱加载失败',",
    replace: "  'inbox.loadFailed': '收件箱加载失败。',",
    why: 'an error message written as a sentence rather than a label',
  },
  {
    gate: 'check-publishable-deps.mjs',
    file: 'sdk/web/package.json',
    find: '"@goliapkg/sentori-core": "^3.1.0"',
    replace: '"@goliapkg/sentori-core": "workspace:*"',
    why: 'a dependency that only resolves inside this monorepo',
  },
  {
    gate: 'check-sdk-version-constant.mjs',
    file: 'sdk/web/src/transport.ts',
    // The declaration, not the number in it. The first version of this
    // probe pinned `'1.0.2'` and stopped matching on the next release,
    // which is the shape of staleness this whole file exists to catch.
    find: 'export const SDK_VERSION = ',
    replace: "export const SDK_VERSION = '0.0.0-probe' && ",
    why: 'an SDK reporting a version it stopped being releases ago',
  },
  {
    gate: 'check-docs-site.mjs',
    file: 'webapp/src/lib/docs.ts',
    find: "  { key: 'docs.section.help', slugs: ['troubleshooting'] },\n",
    replace: '',
    why: 'a docs page the site has no navigation to',
  },
  {
    gate: 'check-error-status.mjs',
    file: 'self-hosted/server/src/handlers/notify_admin.rs',
    find: 'pub async fn smtp_status(State(state): State<Arc<AppState>>) -> Json<Value> {',
    replace:
      'pub async fn smtp_status(State(state): State<Arc<AppState>>) -> Json<Value> {\n' +
      '    if false { return Json(json!({ "error": "injected_probe" })); }',
    why: 'a 200 carrying an error',
  },
];

// How each gate is actually invoked, read from the script that invokes
// it rather than guessed here.
//
// Guessing got it wrong twice: a `.sh` gate was run with node, and the
// webapp checkers are a mix of `node`, `bun` and
// `node --experimental-strip-types` — running one the wrong way gives
// a non-zero exit, which this file would otherwise read as "the gate
// caught the defect". The baseline check below catches that now, but
// only because the invocation is right in the first place.
//
// Two script lists, because there are two: preflight at the root, and
// `check` inside webapp. `check:published-readme` is deliberately not
// among them — it is a release-time gate that needs the network.
function gatesFrom(pkgPath, script, dir) {
  const text = JSON.parse(readFileSync(pkgPath, 'utf8')).scripts[script];
  const out = new Map();
  const re = /(node(?: --[\w-]+)*|bash|bun) ((?:scripts|devtools)\/((?:check|gen)-[a-z0-9-]+\.(?:mjs|sh)))((?: --[\w-]+)*)/g;
  for (const m of text.matchAll(re)) {
    const [runner, ...runnerFlags] = m[1].split(' ');
    out.set(m[3], {
      runner,
      runnerFlags,
      path: m[2],
      args: m[4].trim() ? m[4].trim().split(/\s+/) : [],
      dir,
    });
  }
  return out;
}

const GATES = new Map([
  ...gatesFrom(new URL('../package.json', import.meta.url).pathname, 'preflight', '.'),
  ...gatesFrom(new URL('../webapp/package.json', import.meta.url).pathname, 'check', 'webapp'),
]);

// The tracked files only: the mirror of what a clean checkout holds, and
// small enough to copy in under two seconds.
const copy = mkdtempSync(join(tmpdir(), 'sentori-gates-'));
try {
  execFileSync('sh', ['-c', `git ls-files -z | rsync -0 --files-from=- ./ ${copy}/`], {
    cwd: ROOT,
    stdio: 'pipe',
  });

  // The copy has to be a git repository, because several gates ask git
  // questions: `check-ios-packaging` reads `git ls-files` to learn what
  // a consumer receives, and `check-doc-versions` reads `git tag` to
  // learn which versions are published.
  //
  // Without this they failed here with "not a git repository" — a
  // non-zero exit, which the loop below read as "the gate went red".
  // Both were reported as verified for as long as they have been in
  // this list, having never once run. The baseline check added below
  // is what surfaced it; before that, a gate that could not run and a
  // gate that caught the defect were the same observation.
  // The installed dependencies, borrowed rather than copied.
  //
  // Several checkers import the component they judge, so without
  // node_modules they cannot resolve `react` and do not fail — `bun`
  // sits trying to fetch it. Six gates were carved out as "needs an
  // install" for that reason alone, which meant six gates nothing
  // verified. The install exists on this machine; the sandbox links to
  // it. Nothing writes through the link: probes only ever touch files
  // under the copy.
  const linked = [];
  for (const dir of [
    'node_modules',
    'webapp/node_modules',
    'apps/rn-example/node_modules',
    // Where the workspace puts a peer the SDK declares.
    'sdk/react-native/node_modules',
    'sdk/expo/node_modules',
  ]) {
    const real = join(ROOT, dir);
    if (existsSync(real)) {
      mkdirSync(dirname(join(copy, dir)), { recursive: true });
      symlinkSync(real, join(copy, dir));
      linked.push(dir);
    }
  }

  const git = (...args) =>
    execFileSync('git', args, { cwd: copy, stdio: 'pipe', encoding: 'utf8' });
  git('init', '-q');
  // The linked node_modules must not be walked by the sandbox's own
  // git, which several gates query: `check-surface-gates` reads
  // `git ls-files` and reported `node_modules` as an ungated surface.
  // No trailing slash — the link is a blob to git, not a directory.
  writeFileSync(join(copy, '.git', 'info', 'exclude'), 'node_modules\n**/node_modules\n');
  git('-c', 'user.email=gates@example.com', '-c', 'user.name=gates', 'add', '-A');
  git(
    '-c', 'user.email=gates@example.com', '-c', 'user.name=gates',
    'commit', '-q', '-m', 'sandbox',
  );
  // Tag names only: `check-doc-versions` reads which versions exist,
  // not what they point at.
  for (const tag of execFileSync('git', ['tag', '--list', 'swift/*'], {
    cwd: ROOT,
    encoding: 'utf8',
  })
    .split('\n')
    .filter(Boolean)) {
    git('tag', tag);
  }

  const failures = [];
  const skipped = [];
  for (const p of PROBES) {
    if (p.requires && spawnSync('sh', ['-c', `command -v ${p.requires}`], { stdio: 'ignore' }).status !== 0) {
      skipped.push(`${p.gate} (needs ${p.requires})`);
      continue;
    }
    // A gate that imports the component it judges needs the install.
    // Where there is none — a checkout with no `bun install` — the
    // probe is reported as skipped rather than run, because a gate
    // that cannot resolve its imports fails identically to one that
    // caught its defect.
    if (p.deps && !linked.includes(p.deps)) {
      skipped.push(`${p.gate} (needs ${p.deps}, which is not installed here)`);
      continue;
    }
    const path = join(copy, p.file);
    const before = readFileSync(path, 'utf8');
    if (!before.includes(p.find)) {
      failures.push(
        `${p.gate}: the probe's anchor is gone from ${p.file}. The code moved; ` +
        `re-aim it at where the rule is now, or this gate is untested.`,
      );
      continue;
    }
    // Split, because a gate can take a flag. Passing the whole string
    // as one filename made `node scripts/'gen-replay-vectors.mjs
    // --check'` throw MODULE_NOT_FOUND — a non-zero exit, which this
    // file then read as "the gate went red". That entry had never run
    // the gate at all, and was reported as verified for as long as it
    // has existed. Found by adding a second entry of the same shape.
    const g = GATES.get(p.gate);
    if (!g) {
      failures.push(
        `${p.gate}: no script in package.json runs it, so this probe tests a gate ` +
          `that is not a gate. Wire it into preflight or webapp's check first.`,
      );
      continue;
    }
    const cwd = g.dir === '.' ? copy : join(copy, g.dir);
    const run = () =>
      spawnSync(
        g.runner,
        [...g.runnerFlags, join(cwd, g.path), ...g.args],
        // A gate that cannot resolve its imports does not always
        // fail — `bun` sat for minutes trying to fetch `react` for a
        // checker that imports a component, with no node_modules in
        // the sandbox. A hang and a pass are the same observation from
        // here, so a run that does not finish is a run that did not
        // answer.
        { cwd, encoding: 'utf8', timeout: 60_000 },
      );

    // Green before the probe, or a non-zero exit afterwards says
    // nothing: a gate that cannot run in this sandbox fails either
    // way, and looks exactly like one that caught the defect.
    const baseline = run();
    if (baseline.error?.code === 'ETIMEDOUT') {
      failures.push(
        `${p.gate}: did not finish in 60s on an unmodified tree. It needs something ` +
          `the sandbox does not have — most often node_modules, which a tracked-files ` +
          `copy has no reason to hold.`,
      );
      continue;
    }
    if (baseline.status !== 0) {
      failures.push(
        `${p.gate}: already fails on an unmodified tree, so its red below means ` +
        `nothing. It cannot run here:\n${(baseline.stderr || baseline.stdout || '').trim().slice(0, 400)}`,
      );
      continue;
    }

    writeFileSync(path, before.replace(p.find, p.replace));
    const r = run();
    writeFileSync(path, before);
    if (r.status === 0) {
      failures.push(
        `${p.gate}: stayed green with ${p.why} in ${p.file}. Either the rule ` +
        `no longer reaches that code, or the probe injects something the rule ` +
        `excludes on purpose.`,
      );
    }
  }

  if (failures.length) {
    console.error(`✗ ${failures.length} of ${PROBES.length} gate(s) did not fail on their own defect:`);
    for (const f of failures) console.error(`    ${f}`);
    process.exit(1);
  }
  // Coverage, said out loud.
  //
  // Preflight runs more gates than this file probes, and "every gate
  // went red" reads as "all of them" — the same shape of quiet as the
  // `check-workflow-script-paths` output that said "8 pairs" while
  // skipping three whole workflows. A gate with no probe is not
  // verified; it is merely present. So the count is printed, and so
  // are the names.
  {
    const run = new Set(GATES.keys());
    const probed = new Set(PROBES.map((p) => p.gate));
    // These read a build output or an installed dependency, neither of
    // which is in the sandbox — it copies tracked files. They are checked by preflight and by
    // CI, where a build has happened; they cannot be probed here, and
    // that is a property of the sandbox rather than a gap in them.
    const NEEDS_BUILD = new Set([
      'check-package-entrypoints.mjs',
      'check-sdk-size.sh',
      'check-web-size.sh',
      'check-weapp-size.sh',
      'check-maven-artifact.mjs',
      'check-orphan-lib.mjs',
      // Reads node_modules to learn what we build against, so it needs
      // an install rather than a checkout.
      // Three webapp checkers import the component they judge, so
      // they need webapp/node_modules. They run in preflight and in
      // CI, where an install has happened.
      // Reads every package's built entry point and asks npm what that
      // version number already means, so it needs both a build and the
      // network.
      'check-sibling-exports.mjs',
      // Runs the built `sdk/core/lib/identity.js`, not the TypeScript
      // beside it, so a change to the source is invisible from a
      // tracked-files copy. Preflight builds the SDKs before it.
      'gen-identity-vectors.mjs',
      // Compares sdk/native/VERSION against where `swift/<version>`
      // points. The sandbox is one commit with the tag names copied
      // onto it, so every tag resolves to HEAD and the gate correctly
      // answers "tagged at this commit". Probing it needs real
      // history, not a flattened copy.
      'check-native-version-tag.mjs',
    ]);
    // This file cannot be its own probe: a mutation that makes it go
    // red is a mutation to the thing reporting the result. What stands
    // in for one is the baseline check above — a gate that cannot run
    // in the sandbox is caught before its red is counted, which is how
    // three false passes in this list were found.
    const SELF = 'check-gates-fail.mjs';
    const unprobed = [...run]
      .filter((g) => !probed.has(g) && !NEEDS_BUILD.has(g) && g !== SELF)
      .sort();
    const unprobeable = [...run].filter((g) => !probed.has(g) && NEEDS_BUILD.has(g)).sort();
    const covered = [...run].filter((g) => probed.has(g)).length;
    console.log(
      `✓ ${PROBES.length} gates each went red on the defect they exist for` +
        ` (${covered} of ${run.size} gates preflight and webapp's check run)`,
    );
    if (unprobeable.length > 0) {
      console.log(
        `  ${unprobeable.length} cannot be probed from a tracked-files copy — they need a` +
          ` build, an install or real history: ${unprobeable.join(', ')}`,
      );
    }
    if (unprobed.length > 0) {
      console.log(`  no probe yet, so present rather than verified: ${unprobed.join(', ')}`);
    }
    if (skipped.length > 0) {
      console.log(`  probe skipped, so unverified on this machine: ${skipped.join(', ')}`);
    }
    console.log('  and this file, which cannot be its own probe');
  }
} finally {
  rmSync(copy, { recursive: true, force: true });
}
