# devtools — look at the dashboard before saying it works

Two scripts that render every route locally against canned data and
report what actually happened. Neither runs in production or in CI.

They exist because three shipped bugs in a row — `[object Object]` in
every projects cell, `RangeError` on twelve pages, uuids where the
members list should show emails — were all found by the user opening
the page, and none of them were findable by reading the source. Layout,
wrapping, contrast and "what does this look like when the field is
null" only exist after the thing is composed.

## Use

```bash
bun run mock &                      # :8080, what vite proxies to
bun run build                       # sweep reads dist/, not source
bun run preview &                   # :5599, serves dist/ with the proxy
bun run sweep out zh-CN dark        # <outdir> <lang> <theme>
```

To photograph the error states instead of the happy path, run the mock
with `bun run mock:fail`: every request under the demo project answers
500 with the shape the real server sends. Nothing else renders those
screens, which is how six pages came to show "could not load" and drop
the status and message the server had actually returned.

`sweep.mjs` walks all 12 routes in one Chrome over CDP, writes a PNG per
route into `out/`, and writes `out/report.json` with each route's console
errors and rendered text, under a `bundle` field naming the script it
actually loaded. Read the PNGs; grep the report.

It **exits non-zero** on any console error and on any path the app
requested that the mock did not answer. Both failures render as a
plausible-looking page — an empty state is indistinguishable from
"nothing happened yet" in a screenshot — so neither can be left as a
line of output nobody reads.

```bash
python3 -c "
import json, sys
d = json.load(open(sys.argv[1]))
bad = [r for r in d['routes'] if r['errors']]
print(d['bundle'], d['lang'], d['theme'], '—', len(d['routes']), 'routes,', len(bad), 'with errors')
for r in bad: print(' ', r['route'], r['errors'][0][:80])
for u in d['unmocked']: print('  UNMOCKED', u)
" out/report.json
```

**Preview, not dev, and no editing while it runs.** Two sweeps against
`bun run dev` reported an error that was already fixed, having caught
the editor mid-save; a third ran while `dist/` was being rebuilt
underneath it, so its clean result described a state that never
existed. A sweep is a measurement — rebuild first, then leave the tree
alone until it finishes.

A blank page is nearly always a mock-shape mismatch rather than a bug in
the page — check the route's return type in `src/lib/api.ts` first.

Arguments are `<outdir> <lang> <theme>`; `lang` drives locale
negotiation (`zh-CN` / `ja` / `en`) and `theme` is written to
localStorage before the walk (`dark` / `light`).

Settings sections are in the URL (`settings?tab=audit`), which is what
lets the sweep reach all five of them. Keep it that way when adding a
section: an in-state tab is a screen no sweep can see.

## Keep the dirt in

`mock-api.mjs` deliberately serves bad data alongside good:

- an issue from before the env × platform split, whose `environment`
  and `platform` are still NULL;
- a bare-classname title (`Error`) that the crash view must demote
  behind its message;
- an issue that was resolved and then regressed, and one with no users
  at all;
- a stack frame the symbolicator could not resolve — no line, no
  column, a one-letter name;
- a signal with no `data` at all, an audit row whose actor has been
  deleted, an admin who has never logged in, a revoked token;
- launch percentiles that are `null` rather than `0` because the
  release had three samples and all three were pre-warmed;
- a release with iOS traffic and no dSYM (the light that must be red)
  next to one with no traffic at all (the light that must stay quiet);
- **a replay whose last line is truncated.** Uploads are append-only,
  so a half-written tail is the *expected* damage, and it is likeliest
  in exactly the case replay exists for: the process died. Mapping
  `JSON.parse` over the lines threw the whole recording away for one
  bad byte and the page said "replay failed to load" — found by this
  mock, in this sweep, months after shipping.

Every timestamp in the first version of this file was a valid ISO
string, which is why the screenshots looked fine while production was
throwing. A mock that only produces clean data verifies the happy path
and nothing else. When you add an endpoint here, give it one row that
is missing something.
