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
| Projects | which app, and is it alive at all |

There is no dashboard-of-dashboards and no widget you arrange yourself.
The screens are opinionated because triage is: at 3am you want the next
thing to look at, not a canvas.

## The inbox

The home screen is a split: the queue on the left, the case file on the
right. Selecting a row never leaves the queue, so you can walk a
morning's worth of reports without losing your place.

**The order is the opinion.** Regressed first, then new, then open —
not newest first. Something you already fixed and that came back is
worse news than something you have never seen, and both are worse than
the one you triaged yesterday and left open.

Within that, rows carry the impact: how many people, and the worst
single person's count. `312u×9` is three hundred and twelve people, and
one of them hit it nine times. One user hitting something two hundred
times is a different bug from two hundred users hitting it once, and
the queue will not let you confuse them.

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

Environment, release, and the five kinds. Filters live in the URL, so a
reload lands on the same view and a link you paste into chat opens what
you were looking at.

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
- **Traces** — did the step run, and how long did it take.

This is a status surface, not a data browser. If you need to slice it,
the [protocol](protocol.md) documents the query API.

## Releases

One row per release, with four lights: `js`, `ios`, `android`, `src`.

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

One card per project: health, the last release, the platforms reporting
in the last 24 hours, and replay coverage. This is the layer above
everything else — the switcher in the sidebar picks which project the
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
