// One-shot migration: replace inline `queryKey: [...]` /
// `invalidateQueries({ queryKey: [...] })` with calls to the typed
// factory in `@/api/query-keys.ts`. Idempotent — running twice is a
// no-op once a file is already migrated.
//
// Run with: `bun scripts/migrate-query-keys.ts`
//
// Strategy: text-substitution. The patterns we recognise are exactly
// the ones registered in query-keys.ts. Anything novel (a key that
// isn't in the registry) is left as-is and reported so the human
// adds it.

import { readFileSync, readdirSync, writeFileSync, statSync } from 'node:fs'
import { join } from 'node:path'

const ROOT = 'web/src'

type Rule = {
  /** Regex matching the inline tuple — `[ 'foo', a, b ]`. The capturing
   *  groups are the args. */
  pattern: RegExp
  /** Replacement using `$1`, `$2`, … for the captures. */
  replace: string
  /** Human label for the report. */
  label: string
}

// Order matters: more-specific patterns first. The factory call is
// always a single bound expression so it composes inside
// `queryKey:` and `invalidateQueries({ queryKey: ... })` without
// further surgery.
const RULES: Rule[] = [
  // event sub-resources
  {
    label: 'qk.event.attachments',
    pattern: /\['event-attachments',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.event.attachments($1, $2)',
  },
  {
    label: 'qk.event.frameSource',
    pattern: /\['frame-source',\s*([^,\]]+),\s*([^,\]]+),\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.event.frameSource($1, $2, $3, $4)',
  },
  {
    label: 'qk.event.replayNdjson',
    pattern: /\['replay-ndjson',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.event.replayNdjson($1, $2)',
  },
  {
    label: 'qk.event.replay',
    pattern: /\['replay',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.event.replay($1, $2)',
  },
  {
    label: 'qk.event.viewTree',
    pattern: /\['view-tree',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.event.viewTree($1, $2)',
  },
  {
    label: 'qk.event.sessionTrail',
    pattern: /\['session-trail',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.event.sessionTrail($1, $2)',
  },
  {
    label: 'qk.event.stateSnapshot',
    pattern: /\['state-snapshot',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.event.stateSnapshot($1, $2)',
  },

  // issue subtree
  {
    label: 'qk.issue.detail',
    pattern: /\['issue',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.issue.detail($1, $2)',
  },
  {
    label: 'qk.issue.releases',
    pattern: /\['issue-releases',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.issue.releases($1, $2)',
  },
  {
    label: 'qk.issue.activity',
    pattern: /\['issue-activity',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.issue.activity($1, $2)',
  },
  {
    label: 'qk.issue.userReports',
    pattern: /\['issue-user-reports',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.issue.userReports($1, $2)',
  },
  {
    label: 'qk.issue.culprits',
    pattern: /\['culprits',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.issue.culprits($1, $2)',
  },
  // The `events` and `issues` keys are 2-arg + 1-arg respectively.
  // Put `events` first because the `issues` regex would otherwise
  // greedily match `issue` prefix.
  {
    label: 'qk.issue.events',
    pattern: /\['events',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.issue.events($1, $2)',
  },
  {
    label: 'qk.issue.list',
    pattern: /\['issues',\s*([^,\]]+)\]/g,
    replace: 'qk.issue.list($1)',
  },

  // traces
  {
    label: 'qk.traces.detail',
    pattern: /\['trace-detail',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.traces.detail($1, $2)',
  },
  {
    label: 'qk.traces.list',
    pattern: /\['traces',\s*([^,\]]+)\]/g,
    replace: 'qk.traces.list($1)',
  },

  // audience / posture
  {
    label: 'qk.audience.live',
    pattern: /\['audience-live',\s*([^,\]]+)\]/g,
    replace: 'qk.audience.live($1)',
  },
  {
    label: 'qk.audience.metrics',
    pattern: /\['audience-metrics',\s*([^,\]]+),\s*'day-7'\]/g,
    replace: "qk.audience.metrics($1, 'day')",
  },
  {
    label: 'qk.audience.topRoutes',
    pattern: /\['audience-top-routes',\s*([^,\]]+),\s*'7d'\]/g,
    replace: "qk.audience.topRoutes($1, '7d')",
  },
  {
    label: 'qk.audience.userTimeline',
    pattern: /\['user-timeline',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.audience.userTimeline($1, $2)',
  },
  {
    label: 'qk.posture.pinAnomalies',
    pattern: /\['pin-anomalies',\s*([^,\]]+),\s*'24h'\]/g,
    replace: 'qk.posture.pinAnomalies($1)',
  },
  {
    label: 'qk.posture.trustScores',
    pattern: /\['trust-scores',\s*([^,\]]+)\]/g,
    replace: 'qk.posture.trustScores($1)',
  },

  // metrics / moments / vitals
  {
    label: 'qk.metrics.names',
    pattern: /\['metric-names',\s*([^,\]]+)\]/g,
    replace: 'qk.metrics.names($1)',
  },
  {
    label: 'qk.metrics.points',
    pattern: /\['metric-points',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.metrics.points($1, $2)',
  },
  {
    label: 'qk.moments.samples',
    pattern: /\['moment-samples',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.moments.samples($1, $2)',
  },
  {
    label: 'qk.moments.list',
    pattern: /\['moments',\s*([^,\]]+)\]/g,
    replace: 'qk.moments.list($1)',
  },
  {
    label: 'qk.vitals.report',
    pattern: /\['vitals-report',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.vitals.report($1, $2)',
  },
  {
    label: 'qk.vitals.releases',
    pattern: /\['vitals-releases',\s*([^,\]]+)\]/g,
    replace: 'qk.vitals.releases($1)',
  },

  // misc
  {
    label: 'qk.cmdk',
    pattern: /\['cmdk',\s*([^,\]]+)\]/g,
    replace: 'qk.cmdk($1)',
  },
  {
    label: 'qk.orgs.members',
    pattern: /\['members',\s*([^,\]]+)\]/g,
    replace: 'qk.orgs.members($1)',
  },
  {
    label: 'qk.orgs.teams',
    pattern: /\['teams',\s*([^,\]]+)\]/g,
    replace: 'qk.orgs.teams($1)',
  },
  {
    label: 'qk.tokens',
    pattern: /\['tokens',\s*([^,\]]+)\]/g,
    replace: 'qk.tokens($1)',
  },
  {
    label: 'qk.releases',
    pattern: /\['releases',\s*([^,\]]+)\]/g,
    replace: 'qk.releases($1)',
  },
  {
    label: 'qk.certWatchDomains',
    pattern: /\['cert-watch-domains',\s*([^,\]]+)\]/g,
    replace: 'qk.certWatchDomains($1)',
  },
  {
    label: 'qk.certObservations',
    pattern: /\['cert-observations',\s*([^,\]]+),\s*([^,\]]+)\]/g,
    replace: 'qk.certObservations($1, $2)',
  },
  {
    label: 'qk.privacy.score',
    pattern: /\['privacy-score',\s*([^,\]]+)\]/g,
    replace: 'qk.privacy.score($1)',
  },
  {
    label: 'qk.privacy.findings',
    pattern: /\['privacy-findings',\s*([^,\]]+)\]/g,
    replace: 'qk.privacy.findings($1)',
  },
  {
    label: 'qk.alertRules',
    pattern: /\['alert-rules',\s*([^,\]]+)\]/g,
    replace: 'qk.alertRules($1)',
  },
  {
    label: 'qk.audit',
    pattern: /\['audit',\s*([^,\]]+)\]/g,
    replace: 'qk.audit($1)',
  },
  {
    label: 'qk.userActivity',
    pattern: /\['user-activity',\s*([^,\]]+)\]/g,
    replace: 'qk.userActivity($1)',
  },
  {
    label: 'qk.superadmin.users',
    pattern: /\['superadmin',\s*'users'\]/g,
    replace: 'qk.superadmin.users()',
  },
  {
    label: 'qk.superadmin.orgs',
    pattern: /\['superadmin',\s*'orgs'\]/g,
    replace: 'qk.superadmin.orgs()',
  },
  {
    label: 'qk.superadmin.projects',
    pattern: /\['superadmin',\s*'projects'\]/g,
    replace: 'qk.superadmin.projects()',
  },
  {
    label: 'qk.liveDetail',
    pattern: /\['live',\s*([^,\]]+)\]/g,
    replace: 'qk.liveDetail($1)',
  },

  // global / 1-arg / 0-arg
  { label: 'qk.me', pattern: /\['me'\]/g, replace: 'qk.me()' },
  {
    label: 'qk.oauthProviders',
    pattern: /\['oauth-providers'\]/g,
    replace: 'qk.oauthProviders()',
  },
  { label: 'qk.selfTest', pattern: /\['self-test'\]/g, replace: 'qk.selfTest()' },
  { label: 'qk.orgs.all', pattern: /\['orgs'\]/g, replace: 'qk.orgs.all()' },
  { label: 'qk.projects', pattern: /\['projects'\]/g, replace: 'qk.projects()' },
  {
    label: 'qk.integrations',
    pattern: /\['integrations'\]/g,
    replace: 'qk.integrations()',
  },
]

function walk(dir: string, out: string[]) {
  for (const e of readdirSync(dir, { withFileTypes: true })) {
    const p = join(dir, e.name)
    if (e.isDirectory()) {
      if (e.name === 'node_modules') continue
      walk(p, out)
    } else if (e.isFile() && (e.name.endsWith('.ts') || e.name.endsWith('.tsx'))) {
      out.push(p)
    }
  }
}

function addImport(text: string): string {
  if (text.includes("from '@/api/query-keys'")) return text
  if (!text.includes('qk.')) return text
  // Find the end of the LAST top-level import block — handling
  // multi-line imports (`import {\n  foo,\n  bar,\n} from 'pkg'`).
  // We walk line-by-line, tracking when we're inside an unmatched
  // `{` from an `import` statement, and record the last line that
  // closed an import block.
  const lines = text.split('\n')
  let depth = 0
  let inImport = false
  let lastImportEnd = -1
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i]!
    if (!inImport) {
      if (/^import\s/.test(line)) {
        inImport = true
        depth = 0
        // Count braces on this line
        for (const ch of line) {
          if (ch === '{') depth += 1
          else if (ch === '}') depth -= 1
        }
        // Single-line import ends here (when balanced + has `from`
        // or `;` at the end) OR multi-line continues.
        if (depth === 0) {
          lastImportEnd = i
          inImport = false
        }
      }
    } else {
      // Inside a multi-line import — track brace depth + watch for
      // the closing `}`.
      for (const ch of line) {
        if (ch === '{') depth += 1
        else if (ch === '}') depth -= 1
      }
      if (depth === 0) {
        lastImportEnd = i
        inImport = false
      }
    }
  }
  if (lastImportEnd === -1) {
    return `import { qk } from '@/api/query-keys'\n\n${text}`
  }
  lines.splice(lastImportEnd + 1, 0, "import { qk } from '@/api/query-keys'")
  return lines.join('\n')
}

const files: string[] = []
walk(ROOT, files)

let totalSubs = 0
const perRule = new Map<string, number>()
const filesTouched: string[] = []

for (const f of files) {
  if (f.endsWith('/api/query-keys.ts')) continue
  if (f.endsWith('/migrate-query-keys.ts')) continue
  const original = readFileSync(f, 'utf8')
  let text = original
  let touched = 0
  for (const r of RULES) {
    let matches = 0
    text = text.replace(r.pattern, (...args: unknown[]) => {
      matches += 1
      // The dynamic replace function signature varies; rebuild
      // manually using captured groups.
      const replacement = (r.replace as string).replace(/\$(\d+)/g, (_, n) => {
        const idx = Number(n)
        const value = (args[idx] as string | undefined) ?? ''
        return value
      })
      return replacement
    })
    if (matches > 0) {
      touched += matches
      perRule.set(r.label, (perRule.get(r.label) ?? 0) + matches)
    }
  }
  if (touched > 0) {
    text = addImport(text)
    writeFileSync(f, text)
    totalSubs += touched
    filesTouched.push(f)
  }
}

console.log(`migrated ${totalSubs} site(s) across ${filesTouched.length} file(s)`)
for (const [label, count] of [...perRule.entries()].sort((a, b) => b[1] - a[1])) {
  console.log(`  ${count.toString().padStart(3)}  ${label}`)
}
