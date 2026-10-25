---
title: Using the dashboard
description: Finding the bug — the queue, the case file, and what each screen answers
---

# Using the dashboard

Every screen answers one question. If you know which question you have,
you know which screen to open.

| Screen | The question |
|---|---|
| Inbox | what should I look at next |
| Issue | what happened, to whom, and what led up to it |
| Instruments | are the checks I planted still holding |
| Releases | did this version ship healthy |
| Push | did the message reach the devices |
| Projects | which product and Apps are reporting |

There is no dashboard-of-dashboards and no widget you arrange yourself.
The screens are opinionated because triage is: at 3am you want the next
thing to look at, not a canvas.

## The inbox

The home screen is the issue queue. Open a row to inspect its case file;
Back to issues retains your search, filters and pagination.

The queue defaults to the most recent event first, using the device's
occurrence time rather than the server's receive time. Click a column
heading to sort by issue, kind, status, event count, user count or last
event time; click again to reverse the order. The arrow shows the active
direction. Sorting applies to all matching issues before pagination and
returns you to the first page.

Choose **Impact** in the sort selector to prioritize open regressions,
then the number of affected users and the largest per-user event count.
Search, filters and sorting are preserved when you return from an issue.

### Keyboard

| Key | Does |
|---|---|
| `j` / `k` | walk the queue |
| `Enter` | open the selected issue |
| `x` | mark, for a bulk action |
| `r` | resolve |
| `i` | ignore |
| `⌘K` | jump to a page, or search issues |

### Filters

Filter by App, platform, environment, release, the five event kinds, or
business dimensions. The App type field reads `payload.context.appType`;
additional conditions can use keys from `payload.context` or `payload.tags`.
All conditions must match the **same event**. Queue totals and occurrence
lists reflect the matching events, rather than the entire issue history.

Filters live in the URL, so a reload or shared link preserves the view.
Changing projects clears project-specific dimensions. The crash-free card
uses sessions in the selected App and environment over the last 24 hours;
it does not apply issue, release or event-tag filters.

## The issue

The case file. Top to bottom it reads as the answer to "what happened":

- **The headline** is the message, not the class name. Five rows all
  headed `Error` tell you nothing five times.
- **Impact** — people affected, events, the worst single user, first
  and last seen, which release, which environment.
- **Session replay**, when the SDK sent one: a wireframe of the last
  sixty seconds before the event. Masked fields stay masked; this is
  layout and interaction, not pixels, unless the app opted into
  `replayScreens`.
- **The timeline** — frames, actions, and events on one axis, so you
  can see the taps that led up to the error rather than inferring them.
- **The stack**, symbolicated when the mapping for that release was
  uploaded. Unsymbolicated frames say so rather than showing you a
  plausible wrong line.
- **Occurrences** — every event in this issue, with the platform,
  release, environment and user for each. This is where you find out
  that it only happens on one build, or to one person.

**Resolving anchors to a release.** Resolve in `1.4.0` and the issue
does not come back when an old build reports it again — an old build
is old news. It reopens, marked as a regression, only when something at
or after `1.4.0` reports it.

## Instruments

The three verbs that are not errors, each answering a different
question about code you already wrote:

- **Asserts** — an invariant you expect to hold. The column that
  matters is the failure rate: "ran 45,000, failed 3" is a different
  fact from "ran 3, failed 3".
- **Probes** — a tripwire in code you believe is now unreachable.
  **Silence is the good outcome.** A probe that fires means the path
  you thought you removed is still being taken.
- **Traces** — did the step run, how often, and when was it last seen.

The overview plots received event activity and startup percentiles from
actual reported data. Choose a one-hour, 24-hour, seven-day or 30-day window,
search by name, or filter by release. Chart controls switch to an accessible
data table, hide individual series, zoom the time range and export CSV.
Missing observations remain empty; failed queries show an error and retry.

Assertion and probe counters are cumulative project totals. Their charts
explicitly identify that scope; the time window applies to event activity,
trace events and startup samples. Prewarmed startup samples are counted
separately and excluded from percentiles.

## Releases

Filter versions by app, or view all apps in the project. Each row shows its app
associations and four file indicators: JavaScript, iOS, Android and Source.

- **green** — the mapping is uploaded and readable
- **amber** — uploaded, but something in it cannot be parsed
- **red** — this platform is reporting and the mapping is missing
- **grey** — nothing here uses it

Red is the one that costs you: events are arriving and their stacks
cannot be read. The fix is to upload the mapping — see
[source map upload from CI](recipes/sourcemap-upload.md) — and the
server re-symbolicates the events that already arrived. You do not
lose the crashes that came in before the upload.

Crash-free sessions per release lives here too. A rate is floored, not
rounded: a release with three crashes never reads as 100%.

## Push

Its own module rather than a settings tab, because sending a message to
a fleet is not configuration.

- **Credentials** per provider — APNs, FCM, Web Push. The page shows
  what a key should look like and where it comes from, and tells you
  what it recognised when you paste one.
- **Audience** — count before you send. The same query, not queued.
- **Devices** — what registered, and what was quarantined.
- **Integrate** — the server snippets, in seven languages, for calling
  the send API from your own backend. They name the route the server
  actually serves; a gate keeps them honest.

## Projects

One card per project aggregates health and replay coverage across its apps.
Platforms are discovered from received data and uploaded files. An owner can also
add an App before its first event, using the exact identifier the SDK will report.
App types distinguish Web API from Web App, independently of the runtime platform. Each app shows
its own latest release and optional Git repository; tasks stay at project level.
This is the layer above everything else — the switcher in the sidebar picks which project the
other screens are about.

## Settings

Tokens (ingest and API, scoped), admins and project assignment,
notification preferences, and the audit log.

**The audit log answers "what changed".** Every configuration change,
with who, when, on what, and what they did. "Events stopped flowing" is
sometimes "someone rotated the token an hour ago", and this is the only
screen that can tell you.

Notifications go to email when SMTP is configured, and to a webhook
when a URL is set — the webhook needs no mail server, which is the
configuration most self-hosted instances are actually in.

## Themes and language

Dark by default, light and system available, switchable from the
sidebar. English, 简体中文 and 日本語, following the browser and
overridable. Times render relative with the absolute timestamp on
hover: a device with a wrong clock shows its own opinion of when
something happened, and you can always see ours.
