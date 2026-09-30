"""What the server got after a signalled death, checked against what
`ApplicationExitInfo` is supposed to have told us.

Called by scripts/android-crash-loop.sh. Kept out of the shell script
because a nested heredoc inside a heredoc terminates the outer one.
"""

import json
import subprocess
import sys

# android.app.ApplicationExitInfo.REASON_SIGNALED
REASON_SIGNALED = 2

base, jar, project = sys.argv[1], sys.argv[2], sys.argv[3]


def get(url):
    out = subprocess.run(
        ["curl", "-fsS", "-b", jar, url], capture_output=True, text=True
    ).stdout
    return json.loads(out or "{}")


issues = get(f"{base}/admin/api/issues?projectId={project}").get("issues") or []
seen = []
for issue in issues:
    events = (get(f'{base}/admin/api/issues/{issue["id"]}/events') or {}).get(
        "events"
    ) or []
    for ev in events:
        payload = get(f'{base}/admin/api/events/{ev["id"]}').get("payload") or {}
        exit_info = payload.get("exitInfo")
        err = payload.get("error") or {}
        seen.append(err.get("type"))
        if not exit_info:
            continue
        print(
            f'  {err.get("type")}: source={exit_info.get("source")} '
            f'reason={exit_info.get("reason")}'
        )
        if exit_info.get("source") != "ApplicationExitInfo":
            sys.exit(f'✗ source is {exit_info.get("source")!r}, so this did not '
                     "come from the system's record")
        if exit_info.get("reason") != REASON_SIGNALED:
            sys.exit(f'✗ reason is {exit_info.get("reason")}, expected '
                     f"{REASON_SIGNALED} (REASON_SIGNALED) — the kill was "
                     "classified as something else, and the filter in "
                     "SentoriExitInfo.REPORTED is written for what the system "
                     "actually reports, not for what it was assumed to")
        if err.get("type") != "ProcessSignalled":
            sys.exit(f'✗ type is {err.get("type")!r}, expected ProcessSignalled')
        if not payload.get("nativeCrash"):
            sys.exit("✗ nativeCrash is not set, so the dashboard will file this "
                     "as an error the app reported about itself")
        print("✓ a real ApplicationExitInfo record, read on the next launch and "
              "delivered")
        sys.exit(0)

sys.exit(
    "✗ nothing carrying exitInfo arrived. Event types seen: "
    + (", ".join(repr(t) for t in seen) or "none")
)
