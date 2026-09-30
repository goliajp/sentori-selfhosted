"""What the server got from a real browser, and what the page paid.

Called by scripts/web-live-ingest.sh.
"""

import json
import subprocess
import sys

base, jar, project, out_file = sys.argv[1:5]

# A frame at 60 Hz is 16.7 ms and the project's own red line is 5 ms
# of main thread per tick. `longtask` entries are the browser's own
# definition of "this blocked the page", and the SDK doing 550 calls
# must not produce one.
LONG_TASK_BUDGET_MS = 50

# Per call, in microseconds. A `trace` assembles an event and pushes it
# onto a queue; anything approaching a millisecond means something is
# doing real work on the caller's thread.
PER_CALL_BUDGET_US = 500


def get(path):
    out = subprocess.run(
        ["curl", "-fsS", "-b", jar, f"{base}{path}"], capture_output=True, text=True
    ).stdout
    return json.loads(out or "{}")


issues = get(f"/admin/api/issues?projectId={project}&limit=50").get("issues") or []
if not issues:
    sys.exit("✗ a page that threw twice and clicked once produced no issues")

titles = [i.get("title") or "" for i in issues]


def want(fragment, why):
    if not any(fragment in t for t in titles):
        sys.exit(f"✗ no issue titled like {fragment!r} — {why}.\n  got: {titles}")


want("TypeError", "an uncaught error thrown from a timeout did not arrive")
want("RangeError", "an unhandled promise rejection did not arrive")

# The breadcrumbs. They are the reason to have a ring at all, and they
# are also the thing most likely to leak: a click breadcrumb that
# carries the button's text ships whatever the button said.
detail = None
for issue in issues:
    if "TypeError" not in (issue.get("title") or ""):
        continue
    events = (get(f'/admin/api/issues/{issue["id"]}/events') or {}).get("events") or []
    if events:
        detail = get(f'/admin/api/events/{events[0]["id"]}')
    break

if detail is None:
    sys.exit("✗ the uncaught TypeError has no stored event behind it")

payload = detail.get("payload") or {}
signals = payload.get("signals") or []
kinds = {s.get("kind") for s in signals}
if "click" not in kinds:
    sys.exit(f"✗ the click never reached the signal ring; kinds present: {sorted(kinds)}")
if "nav" not in kinds:
    sys.exit(
        "✗ a history.pushState navigation is missing from the ring — a single-page "
        f"app's navigations would be invisible; kinds present: {sorted(kinds)}"
    )

blob = json.dumps(signals)
for leaked in ("Dora Cawley", "412.00"):
    if leaked in blob:
        sys.exit(
            f"✗ {leaked!r} travelled in a breadcrumb — the button's label is user "
            "content, and a breadcrumb is context, not a reason to ship it"
        )

if not detail.get("userKey"):
    sys.exit("✗ the event arrived with no userKey, so breadth cannot be counted")
if (payload.get("device") or {}).get("os") in (None, ""):
    sys.exit("✗ no device was recorded")

# ── the wireframe replay ──────────────────────────────────────────
# Shape, never content. A wireframe that carried the words would be a
# screenshot with extra steps, and the point of this mode is that it is
# the one you can leave on.
atts = detail.get("attachments") or []
replay = next((a for a in atts if a.get("kind") == "replay"), None)
if replay is None:
    sys.exit(
        "✗ the error carries no replay attachment, though the page enabled it — "
        f"attachments present: {[a.get('kind') for a in atts]}"
    )

raw = subprocess.run(
    ["curl", "-fsS", "-b", jar, f'{base}/admin/api/attachments/{replay["ref"]}'],
    capture_output=True,
    text=True,
).stdout
lines = [l for l in raw.splitlines() if l.strip()]
if not lines:
    sys.exit("✗ the replay attachment is empty")

first = json.loads(lines[0])
if first.get("kind") != "key":
    sys.exit(f'✗ the replay starts with a {first.get("kind")!r}, not a keyframe — a player '
             "joining here has nothing to reconstruct against")
if len(first.get("nodes") or []) < 5:
    sys.exit(
        f'✗ the first keyframe has {len(first.get("nodes") or [])} node(s); the harness page '
        "has a card, a heading, a paragraph, an image, an input and a button, so a walker "
        "finding almost nothing is a walker that is not working"
    )

kinds = {n.get("kind") for n in first["nodes"]}
for needed in ("text", "image", "input", "button"):
    if needed not in kinds:
        sys.exit(f"✗ no {needed!r} node in the wireframe; kinds found: {sorted(k for k in kinds if k)}")
if "mask" not in kinds:
    sys.exit("✗ the registered mask query produced no masked node, so `.secret` was walked "
             "like anything else")

# The whole claim, checked against the page's actual strings.
for leaked in ("Dora Cawley", "412.00", "dora@example.com", "40-11-22", "87654321", "Order 4471"):
    if leaked in raw:
        sys.exit(f"✗ {leaked!r} is inside the replay attachment — the wireframe is carrying "
                 "content, not shape")

masked_nodes = [n for n in first["nodes"] if n.get("kind") == "mask"]
if any(n.get("text") for n in masked_nodes):
    sys.exit("✗ a masked node carries a text length, which leaks how much was written there")

cost = json.load(open(out_file))
long_tasks = cost.get("longTasks") or []
over = [d for d in long_tasks if d > LONG_TASK_BUDGET_MS]
if over:
    sys.exit(
        f"✗ using the SDK produced long tasks of {over} ms (budget {LONG_TASK_BUDGET_MS} ms). "
        "A reporter that costs the page a frame is one the host removes."
    )

# And the per-call cost, which is what the long-task check is really
# about — a page that calls this from a render path pays it per call.
# Generous on purpose: the number to catch is a regression of an order
# of magnitude, not the difference between two runners.
per_call_us = (cost.get("sdkMs", 0) / max(1, cost.get("calls", 1))) * 1000
if per_call_us > PER_CALL_BUDGET_US:
    sys.exit(
        f"✗ an SDK call costs {per_call_us:.0f} µs (budget {PER_CALL_BUDGET_US} µs) — "
        f"{cost.get('sdkMs', 0):.0f} ms over {cost.get('calls')} calls"
    )

print(
    f"✓ a real browser: {len(issues)} issues, breadcrumbs without the label's text, "
    f'{len(first["nodes"])} wireframe nodes carrying no page content, '
    f"{len(long_tasks)} long task(s), {per_call_us:.0f} µs per SDK call"
)
