// Read a sweep's rendered text and flag English that is still English.
//
// `check-i18n.mjs` proves every key is wired up; it cannot prove a
// screen has no hard-coded string left, because a string that was never
// given a key is invisible to it. This reads what the pages actually
// rendered and looks for English prose in a non-English run.
//
// It reports rather than gates: the allow-list below can never be
// complete — product names, protocol nouns, code samples and whatever
// the mock happens to contain are all legitimately English — so a hit
// is a question, not a verdict. Every pass so far has found something
// real, and the noise has been obvious at a glance.
//
//   node devtools/check-sweep-text.mjs ../tmp/sweep/report.json
import { readFileSync } from 'node:fs';

const path = process.argv[2];
if (!path) {
  console.error('usage: check-sweep-text.mjs <report.json>');
  process.exit(2);
}

// Legitimately English in any locale: our own names, other companies',
// protocol and file-format nouns, HTTP verbs, code and paths, the
// language switcher, and the fixtures the mock serves.
const ALLOWED = new RegExp(
  [
    'Sentori|SaaS|GOLIA|golia\\.jp|takagi',
    'Slack|Linear|Jira|GitHub|GitLab|Google|Stripe|Acme',
    'APNs|FCM|WebPush|HCM|MiPush|VAPID|Prometheus|Webhook|SMTP|OAuth',
    'sourcemap|dsym|proguard|NDJSON|JSON|CSV|UUID|SDK|API|URL|IP|CLI',
    'GET|POST|PUT|PATCH|DELETE|HEAD',
    'healthz|livez|readyz|metrics|describe',
    'English|简体中文|日本語',
    'Mozilla|Macintosh|Windows|Android|Chrome|Safari|Firefox',
    // The health page lists workers, env vars and auth schemes by their
    // real names on purpose — an operator greps for `Bearer`, not for
    // its translation.
    'Bearer|HttpOnly|Cookie|ES256|RS256|JWT|TLS|HTTP|TCP|Postgres|Valkey',
    // fixture values from devtools/mock-api.mjs
    'insight|myapp|TypeError|NetworkError|RangeError|Cannot read|Request timed',
    'Maximum call|production|staging|Checkout|Cart|Home|Pay now|Unresolved',
    'Crash|Events at|checkout|payments',
    "Let's Encrypt|Encrypt|BadDeviceToken|Send",  // CA and provider strings from fixtures
    'Ctrl|Cmd|Shift|Enter|Esc|Tab',               // key names, printed as typed
  ].join('|'),
);

// Two or more English words, or one long one. Sentence case only —
// lowercase identifiers and ALLCAPS constants are not prose.
const PROSE = /\b[A-Z][a-z]{2,}(?:\s+[a-z]{2,}){0,5}\b/g;

const data = JSON.parse(readFileSync(path, 'utf8'));
const routes = Array.isArray(data) ? data : data.routes;
const locale = data.lang ?? 'unknown';

if (locale.startsWith('en')) {
  console.log(`(${path} is an ${locale} run — nothing to check)`);
  process.exit(0);
}

const hits = new Map();
for (const r of routes) {
  for (const line of (r.text ?? '').split('\n')) {
    for (const m of line.trim().matchAll(PROSE)) {
      const phrase = m[0];
      if (phrase.length <= 3 || ALLOWED.test(phrase)) continue;
      if (!hits.has(r.route)) hits.set(r.route, new Set());
      hits.get(r.route).add(phrase);
    }
  }
}

if (!hits.size) {
  console.log(`✓ no untranslated prose in ${routes.length} ${locale} routes`);
  process.exit(0);
}
let total = 0;
for (const [route, phrases] of [...hits].sort()) {
  total += phrases.size;
  console.log(`${route}\n    ${[...phrases].sort().join(' | ')}`);
}
console.log(
  `\n${total} phrase(s) across ${hits.size} route(s) — each is a question: ` +
    `hard-coded string, or legitimately English?`,
);
