"""What the server got from the mini-program SDK.

Called by scripts/weapp-live-ingest.sh.
"""

import json
import subprocess
import sys

base, jar, project = sys.argv[1:4]


def get(path):
    out = subprocess.run(
        ["curl", "-fsS", "-b", jar, f"{base}{path}"], capture_output=True, text=True
    ).stdout
    return json.loads(out or "{}")


issues = get(f"/admin/api/issues?projectId={project}&limit=50").get("issues") or []
if not issues:
    sys.exit("✗ five reported failures produced no issues")

titles = [i.get("title") or "" for i in issues]


def want(fragment, why):
    if not any(fragment in t for t in titles):
        sys.exit(f"✗ nothing titled like {fragment!r} — {why}\n  got: {titles}")


# `wx.onError` hands a flattened string. If the head is not split back
# into a type and a message, every mini-program crash fingerprints as
# one issue called `Error`.
want("TypeError", "the string wx.onError hands over was not split into a type and a message")
want("RangeError", "an unhandled rejection did not arrive")
# The three that exist nowhere else in this product.
want("weapp.pageNotFound", "a route that does not exist went unreported")
want("weapp.memoryWarning", "the system asking for memory back went unreported")
want("weapp.lazyLoadError", "a subpackage that would not load went unreported")

detail = None
for issue in issues:
    if "TypeError" not in (issue.get("title") or ""):
        continue
    events = (get(f'/admin/api/issues/{issue["id"]}/events') or {}).get("events") or []
    if events:
        detail = get(f'/admin/api/events/{events[0]["id"]}')
    break
if detail is None:
    sys.exit("✗ the TypeError has no stored event behind it")

if detail.get("platform") != "weapp":
    sys.exit(f'✗ stored platform is {detail.get("platform")!r}, want "weapp" — either the SDK '
             "sends the wrong value or the server's vocabulary is behind")

# The identity hash. There is no `crypto.subtle` in this runtime, and
# until the pure-JS fallback existed `hashIdentities` threw here,
# `setUser` swallowed it, and every event shipped with no userKey —
# breadth reading zero with nothing saying why.
key = detail.get("userKey")
if not key:
    sys.exit("✗ the event has no userKey, so this runtime is not hashing identities at all")
if len(key) != 64:
    sys.exit(f"✗ userKey is {len(key)} characters, not a sha256 hex digest")

payload = detail.get("payload") or {}
stack = (payload.get("error") or {}).get("stack") or []
if len(stack) < 2:
    sys.exit(
        f"✗ the stack has {len(stack)} frame(s); wx.onError's blob carried two, so it was "
        "not parsed back out and every mini-program crash arrives without a location"
    )
top = stack[0]
if "pages/cart/index.js" not in (top.get("file") or ""):
    sys.exit(f'✗ the top frame is {top.get("file")!r}, not the file the blob named')

if (payload.get("device") or {}).get("model") in (None, ""):
    sys.exit("✗ no device was recorded; wx.getSystemInfoSync was never read")

print(
    f"✓ a wx shim against a real server: {len(issues)} issues including all three "
    f"platform-only failures, a {len(stack)}-frame stack parsed out of wx.onError's "
    "string, and a hashed userKey without crypto.subtle"
)
