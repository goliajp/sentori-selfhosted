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

`sweep.mjs` walks all 30 routes in one Chrome over CDP, writes a PNG per
route into `out/`, and writes `out/report.json` with each route's console
errors and rendered text, under a `bundle` field naming the script it
actually loaded. Read the PNGs; grep the report.

```bash
python3 -c "
import json, sys
d = json.load(open(sys.argv[1]))
bad = [r for r in d['routes'] if r['errors']]
print(d['bundle'], d['lang'], d['theme'], '—', len(d['routes']), 'routes,', len(bad), 'with errors')
for r in bad: print(' ', r['route'], r['errors'][0][:80])
" out/report.json
```

**Preview, not dev, and no editing while it runs.** Two sweeps against
`bun run dev` reported an error that was already fixed, having caught
the editor mid-save; a third ran while `dist/` was being rebuilt
underneath it, so its clean result described a state that never
existed. A sweep is a measurement — rebuild first, then leave the tree
alone until it finishes.

A blank page is nearly always a mock-shape mismatch rather than a bug in
the page — check the route's return type in `src/lib/api.ts` first. The
mock logs `UNMOCKED <path>` for anything it does not recognise.

Arguments are `<outdir> <lang> <theme>`; `lang` drives locale
negotiation (`zh-CN` / `ja` / `en`) and `theme` is written to
localStorage before the walk (`dark` / `light`).

## Keep the dirt in

`mock-api.mjs` deliberately serves bad data alongside good:

- `regressed_at` arrives as `[1970,1,0,...]` — `time`'s default
  `Serialize`, the exact shape that made `Intl.RelativeTimeFormat`
  throw and took twelve pages down in v1.7.15.
- nullable columns (`last_used_at`, `next_attempt_at`, `deploy_at`,
  `last_bucket`) are actually null somewhere.
- one member has an unverified email, one token is revoked, one push
  send has failed, one workspace is suspended.

Every timestamp in the first version of this file was a valid ISO
string, which is why the screenshots looked fine while production was
throwing. A mock that only produces clean data verifies the happy path
and nothing else. When you add an endpoint here, give it one row that
is missing something.
