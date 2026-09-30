"""What the replay driver left after running against a real screen.

The ring is unit-tested against generated frames and the timer is
unit-tested for not being on the main queue. The capture between them
reads the view hierarchy, which is the reason the timer must not be on
main — and it had never run outside a simulator screenshot.
"""
import json
import sys

lines = [l for l in open(sys.argv[1]).read().splitlines() if l.strip()]
if not lines:
    sys.exit('✗ the drain file is empty')
first = json.loads(lines[0])
if first.get('kind') != 'key':
    sys.exit(f'✗ the first entry is {first.get("kind")!r}, not a keyframe')
nodes = first.get('nodes') or []
# More than the root. A capture that returned only the window would
# pass a "has nodes" check and have walked nothing; the harness screen
# has a stack, three colour blocks, two labels and a button behind it.
if len(nodes) < 4:
    sys.exit(
        f'✗ the keyframe has {len(nodes)} node(s) — the capture saw the window '
        'and not the tree inside it'
    )
print(f'  {len(lines)} entries, first keyframe has {len(nodes)} nodes '
      f'({first.get("width")}x{first.get("height")})')
print('✓ the replay driver captured a real screen')
