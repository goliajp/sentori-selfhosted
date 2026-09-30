"""Put Maven Central's warnings where a person will see them.

    python3 scripts/lib/central-quota-notice.py <stateJson> [summaryFile]

The organisation has a monthly release quota. The run that published
2.1.0 said "6 of 7 used (86%)" as one line inside a step's log, which
is where a release blocker goes to not be read. A publish that fails
next month for a reason printed this month is a preventable surprise.

`::warning::` pins it to the top of the run page; the step summary
puts it on the page somebody lands on after clicking a green check.
Neither fails the build — being near a quota is not a reason to refuse
a release that is otherwise correct.
"""

import json
import sys

state_file = sys.argv[1]
summary = sys.argv[2] if len(sys.argv) > 2 else ""

try:
    with open(state_file, encoding="utf-8") as f:
        warnings = json.load(f).get("warnings") or []
except (OSError, ValueError):
    # The caller already printed the raw response on a bad state; this
    # is a notice, not a gate.
    warnings = []

for w in warnings:
    print(f"::warning title=Maven Central::{w}")
    if summary:
        with open(summary, "a", encoding="utf-8") as f:
            f.write(f"> **Maven Central**: {w}\n\n")
