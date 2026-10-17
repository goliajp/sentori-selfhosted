# Web App monitoring: research and implementation contract

Research date: 2026-10-17. Status: proposed implementation; this document does not claim these capabilities are deployed. It extends the project/app separation work. Public setup documentation must describe only features verified in a released version.

## Product boundary

A project owns apps; an app owns releases, issues, measurements and monitor configurations. App kind describes its role (`ios`, `android`, `webapi`, `webapp`); telemetry platform still describes the runtime. A browser-rendered administration app and a JavaScript API service must not be merged because both run JavaScript.

For Insight, retain the requested three projects and six identified apps:

| Project | Apps | Web monitoring policy |
| --- | --- | --- |
| `insight` | iOS, Android | Historical Expo events remain attached to the corresponding mobile app; Expo does not imply a website. |
| `insight-home` | iOS, Android | Mobile telemetry; no automatic website crawl. |
| `insight-dev-server` | `api` (`webapi`), `admin-panel` (`webapp`) | API health monitoring plus authenticated browser journeys for the admin panel. |

Public websites can use the same webapp capability, with an explicit public/indexable policy. The admin panel is private: lack of public indexing is expected, not an SEO incident. Authentication is the access boundary; robots directives do not replace it.

## Existing behavior and gaps

Audited at source baseline `781debcf`, before this feature's implementation:

- `sdk/web/src/handlers/uncaught.ts` captures window errors and unhandled rejections. This observes executed failures; it cannot establish that every route or user journey is bug-free.
- `handlers/web-vitals.ts` writes measurements into the error-context signal ring. It does not independently report healthy page visits. Its CLS is a lifetime sum and its INP is the worst observed interaction, so those values must not be presented as standards-compliant Core Web Vitals.
- `handlers/network.ts` observes fetch/XHR timing breadcrumbs. It does not provide portable HTTP status coverage or a complete resource waterfall.
- No end-to-end web performance aggregation or SEO monitor was found in the inspected server, CLI, dashboard and docs. Existing error reporting and replay are foundations, not evidence of those capabilities.

## Three complementary data sources

| Source | Answers | Limits |
| --- | --- | --- |
| Browser SDK, real-user monitoring (RUM) | Which actual visits were slow or failed, on which route/device/release? | Only observed, sampled visits; browser APIs and cross-origin restrictions limit visibility. |
| Isolated browser runner | Does a configured URL or journey load, render and work under controlled conditions? | Tested paths and runner conditions only; not representative user percentiles. |
| HTTP/rendered-page SEO audit | Are public pages technically crawlable and correctly described? | Cannot prove Google indexed the URL or promise ranking. |

Keep the source visible in storage, CLI and UI. Never combine synthetic measurements and real-user samples into one percentile.

## Real-user performance

Use the official `web-vitals` implementation for standard LCP, INP and CLS lifecycle handling, subject to the repository's bundle and runtime cost gates. Prefer an optional import/entry point for richer attribution. A handwritten approximation must have a different metric name.

At the 75th percentile, the good thresholds are LCP ≤ 2.5 seconds, INP ≤ 200 milliseconds and CLS ≤ 0.1; segment mobile and desktop. Also capture TTFB and FCP to help locate loading delays. [Google Web Vitals](https://web.dev/articles/vitals), [reference implementation](https://github.com/GoogleChrome/web-vitals).

Collect bounded diagnostic context: document DNS/connect/TLS/request/response phases where exposed, slow resource summaries, resource type/size where available, and long-task attribution where supported. A hidden cross-origin timing field is unknown, not zero milliseconds; `Timing-Allow-Origin` controls access. [Resource Timing](https://developer.mozilla.org/en-US/docs/Web/API/PerformanceResourceTiming).

Separate full document navigation, back/forward cache restoration and SPA route transitions. Use explicit router hooks for route-ready timing; do not reset document-level metrics on every route and label them standard route INP/LCP. Record capability and missing-value reasons, including no user interaction, unsupported API and insufficient samples.

Performance collection must be explicit opt-in, with a configurable stable page-view sampling decision and a bounded batch queue. Include healthy sampled visits; reporting only slow visits corrupts percentiles. Store sample rate, page-view identity, metric identity/revision, release, normalized route, environment and device class. Deduplicate updates before aggregation, and flush on supported page lifecycle transitions without claiming delivery is guaranteed. Avoid averages of per-window percentiles. Display sample counts, time window and coverage; do not show a green pass for absent data.

## JavaScript and user-visible failures

Retain uncaught/rejection capture and sourcemap symbolication by exact app/release. Add resource-load failure capture separately from JavaScript exceptions (script/chunk/CSS/image). Integrate framework error boundaries explicitly for handled render failures; use host-provided readiness assertions for blank screens and critical route failures. A DOM-size heuristic alone must not create a confirmed blank-screen bug.

Network failure reporting needs a defined, tested policy: browser timing alone cannot guarantee status availability; optional instrumentation must preserve host request behavior, ignore Sentori transport, and never read request or response bodies by default. Distinguish aborted requests, connectivity failures, HTTP errors and opaque responses.

Error views should expose affected visits, route, release, browser, first/last occurrence and symbolication status. Error-rate denominators must use compatible sampling; never divide all errors by sampled visits without correction. Console messages are supplementary evidence, not automatically confirmed application bugs. [Window errors](https://developer.mozilla.org/en-US/docs/Web/API/Window/error_event), [unhandled rejections](https://developer.mozilla.org/en-US/docs/Web/API/Window/unhandledrejection_event).

## Active browser checks

Run Chromium/Lighthouse in an isolated runner rather than inside an ingest request or the end-user SDK. Check navigation, expected page readiness, console exceptions, failed resources, redirect loops and specified interactions. Record device/network profile, browser/tool versions, timestamps and checked route/journey. Add screenshots and traces only with bounded retention and redaction suitable for the app.

Support manual and deployment-triggered runs first; recurring schedules require explicit configuration. Use a dedicated test account for private admin journeys, with runner-side credentials and read-only actions. An expired test login is a monitor authentication failure, not proof that production is unavailable.

Lighthouse provides laboratory performance and SEO audits. Its default navigation run does not measure real-user INP; TBT is a diagnostic proxy and must stay labeled TBT. Lab regressions need repeat measurements under a stable profile, separate from RUM alerts. [Lighthouse](https://developer.chrome.com/docs/lighthouse/overview), [lab versus field metrics](https://web.dev/articles/vitals).

Crawler execution must enforce configured hosts, redirect destinations, resource limits and timeouts. Prevent requests to metadata/internal networks from public target configuration; private targets need an explicitly trusted runner. Apply destination checks to subresources as well as the starting URL. Do not expose arbitrary shell commands or arbitrary browser scripts through ordinary project tokens.

## Technical SEO

For public/indexable pages, compare response HTML and rendered content: HTTP status, redirects, title/description presence, canonical target, robots.txt, robots meta/X-Robots-Tag, crawlable links, sitemap entries and broken internal links. Check hreflang and structured-data syntax where configured. Report findings with the URL, rule, observed evidence and repair guidance, rather than only an aggregate score.

JavaScript rendering and blocked resources can affect what search engines see. A valid response or a passed local audit does not establish indexing. [Google JavaScript SEO](https://developers.google.com/search/docs/crawling-indexing/javascript/javascript-seo-basics).

For private/noindex apps, test the configured access policy and applicable noindex directives instead. A robots.txt disallow is not a reliable substitute for noindex: a crawler must be able to read a noindex directive to honor it. [Google indexing controls](https://developers.google.com/search/docs/crawling-indexing/block-indexing).

Search Console integration is a later, separately authorized capability. Its URL Inspection API reports Google's indexed version and requires access to the property; it is not a local live SEO test. [URL Inspection API](https://developers.google.com/webmaster-tools/v1/urlInspection.index/inspect).

## Storage, CLI and dashboard contract

All writes and reads enforce the project/app relationship; an app ID from another project must fail closed. Use explicit app identity for both browser SDK and monitors. Never infer the API versus admin app from the common project token or JavaScript platform alone.

Store raw bounded performance samples and monitor runs separately from issue events. Preserve the existing five event kinds; derive deduplicated actionable issues from configured rules with minimum sample sizes, persistence and recovery behavior. Store audit rule version, evidence and source so a runner upgrade does not masquerade as a site regression.

The dashboard should provide app-scoped Errors, Performance and Checks views, with release/environment/route filters. CLI capabilities should cover app discovery, performance queries, monitor configuration, explicit run initiation and run/finding inspection. These are requirements, not published command names. JSON output must carry units, source, sample count, coverage and timestamps. A normal successful command can return unhealthy findings; a documented CI strict mode should separately signal failed checks. Authentication/transport failures must be distinguishable from site failures.

URL normalization must remove credentials, queries and fragments and allow route templates to avoid collecting identifiers from paths. Do not collect form contents, authorization headers or DOM text for performance attribution. Cap resource count, labels, payload bytes, batches, stored evidence and per-project runner concurrency. Keep SDK remote disable/failure isolation and verify the repository's CPU, rendering, network and bundle budgets before enabling new code.

## Acceptance and rollout

1. Preserve the requested project/app split, Expo lineage, releases and symbols; configure the API health endpoint and admin app identity in the Insight handoff.
2. Implement standards-compliant opt-in RUM, ingestion, app-scoped query/UI/CLI and meaningful samples/coverage before advertising a performance dashboard.
3. Extend error/resource diagnostics with sourcemap and lifecycle fixtures. Test unsupported browsers, no interaction, bfcache, SPA navigation, duplicate flushes, offline queues and cross-project rejection.
4. Add isolated explicit browser/SEO runs with fixtures for slow pages, thrown/rejected JS, broken chunks, canonical/noindex/robots conflicts, login expiry and blocked destinations. Verify CLI findings agree with the dashboard.
5. Prove host overhead with SDK-on/off rendering, CPU and network measurements under the shared benchmark lock. Then run the remote release acceptance gates and publish exact supported versions in website/docs.

The Insight handoff must state what is live, what is opt-in and what remains planned. Do not instruct insight-mono to call proposed configuration keys or CLI commands before their release is verified.
