"""Two source maps with the same basename; the right one must win.

    python3 scripts/lib/two-maps-check.py <base> <cookieJar> <adminToken> <ingestToken> <release> <distDir>

A WeChat mini program names every page's entry `index.js` and a web
build names every route chunk after its route, so a basename is not an
identifier. The server ranks candidate maps by how many trailing path
segments they share with the frame — written for exactly this, and
until now nothing exercised it, because a release only ever held one
map.

It could not have been exercised either: `sentori-cli upload sourcemap`
sent `basename(path)`, so both of these would have arrived as
`index.js.map` and the path was gone before the server saw it. That is
fixed alongside this check.

Written in Python rather than added to the shell script it is called
from: the assertion needs a real column from a real map, and building
that in bash meant six lines of escaped JSON inside a command
substitution inside a string.
"""

import json
import subprocess
import sys
import urllib.parse
import uuid

base, jar, admin_token, ingest_token, release, dist = sys.argv[1:7]


def run(*args: str) -> str:
    return subprocess.run(list(args), capture_output=True, text=True, check=False).stdout


def upload(page: str) -> None:
    path = f"{dist}/pages/{page}/index.js.map"
    code = run(
        "curl", "-sS", "-o", "/dev/null", "-w", "%{http_code}",
        "-X", "POST",
        f"{base}/v1/releases/{urllib.parse.quote(release, safe='')}/artifacts",
        "-H", f"Authorization: Bearer {admin_token}",
        "-F", "kind=sourcemap",
        # The name carries the directory. Without it both arrive as
        # `index.js.map` and the server has nothing to rank on.
        "-F", f"file=@{path};filename=pages/{page}/index.js.map",
    )
    if code not in ("200", "201"):
        sys.exit(f"✗ uploading pages/{page}/index.js.map answered {code}")


for page in ("cart", "home"):
    upload(page)
print("      uploaded two maps, both named index.js.map")

# A position the cart map genuinely covers: the first mapping in it,
# read out of the map rather than guessed. A made-up column resolves to
# nothing and the test would fail for the wrong reason.
with open(f"{dist}/pages/cart/index.js.map", encoding="utf-8") as f:
    cart_map = json.load(f)
segment = cart_map["mappings"].split(";")[0].split(",")[0]
if not segment:
    sys.exit("✗ the cart map's first line has no mappings — the fixture stopped being one")


def vlq(s: str) -> list[int]:
    alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    out, shift, value = [], 0, 0
    for ch in s:
        d = alphabet.index(ch)
        value += (d & 31) << shift
        if d & 32:
            shift += 5
            continue
        negative = value & 1
        value >>= 1
        out.append(-value if negative else value)
        shift = value = 0
    return out


generated_column = vlq(segment)[0]

event_id = str(uuid.uuid4())
body = {
    "id": event_id,
    "kind": "error",
    "occurredAt": "2026-09-30T00:00:00Z",
    "platform": "weapp",
    "release": release,
    "environment": "test",
    "payload": {
        "error": {
            "type": "Error",
            "message": "cart boom",
            # The file names the directory, which is the only thing
            # that can tell the two maps apart.
            "stack": [
                {
                    "file": "pages/cart/index.js",
                    "function": "x",
                    "line": 1,
                    "column": generated_column + 1,
                    "inApp": True,
                }
            ],
        }
    },
}
code = run(
    "curl", "-sS", "-o", "/dev/null", "-w", "%{http_code}",
    "-X", "POST", f"{base}/v1/events",
    "-H", f"Authorization: Bearer {ingest_token}",
    "-H", "Content-Type: application/json",
    "--data-raw", json.dumps(body),
)
if code != "202":
    sys.exit(f"✗ ingesting the cart frame answered {code}")

import time

time.sleep(1)
stored = json.loads(run("curl", "-sS", "-b", jar, f"{base}/admin/api/events/{event_id}") or "{}")
frames = (stored.get("payload") or {}).get("error", {}).get("stack") or []
hit = next((f for f in frames if f.get("symbolicated")), None)
if hit is None:
    sys.exit(
        "✗ neither map resolved a frame from pages/cart/index.js — with two maps in "
        f"the release the server resolved none. frames: {json.dumps(frames)[:300]}"
    )

resolved = str(hit.get("file"))
print(f"      resolved to {resolved}")
# `pages/home`, not `home`. The resolved path is relative to the map's
# sourceRoot and on a GitHub runner that walks up into
# `../../../../home/runner/work/...`, so a bare `home` matched the
# runner's home directory and failed a passing case on CI while passing
# on a Mac. The page directory is the thing being distinguished, so it
# is the thing to look for.
if "pages/home" in resolved:
    sys.exit(
        "✗ the frame resolved against pages/home. The server matched on the basename "
        "and picked the wrong file, and a plausible line in the wrong source is worse "
        "than no line: nothing about it says so."
    )
if "pages/cart" not in resolved:
    sys.exit(f"✗ resolved to {resolved}, which names neither page")

print("✓ two maps named index.js.map, and the frame resolved against the right one")
