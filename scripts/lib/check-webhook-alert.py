"""What the webhook receiver was actually sent."""

import json
import sys

hook_file, server_log = sys.argv[1], sys.argv[2]

try:
    with open(hook_file, encoding="utf-8") as f:
        received = json.load(f)
except (OSError, ValueError):
    received = []

if not received:
    print("── server log ──", file=sys.stderr)
    print(open(server_log, encoding="utf-8").read()[-2500:], file=sys.stderr)
    sys.exit(
        "✗ a new issue was created and nothing POSTed to the webhook. With no SMTP "
        "this instance now has no alert channel at all, which is the state this "
        "column was added to end."
    )

first = received[0]
if first["path"] != "/team-room":
    sys.exit(f'✗ posted to {first["path"]!r}, not the path the URL named')

body = first["body"]
for field in ("subject", "body"):
    if not body.get(field):
        sys.exit(f"✗ the payload has no {field!r}: {json.dumps(body)[:300]}")

# The subject is what a person reads in the room. An alert that does
# not name the error is one nobody can triage from.
if "PaymentDeclined" not in json.dumps(body):
    sys.exit(f"✗ neither the subject nor the body names the error: {json.dumps(body)[:300]}")

print(
    f'✓ a new issue POSTed to the webhook: {first["path"]} '
    f'— {body["subject"][:60]!r}'
)
