// Two labels must not print on top of each other.
//
// The timeline's 0s mark is labelled with a phrase, not a number, and
// it hangs to the left of the event line — so the tick immediately
// before it is the one it lands on. At a 60s window with a 5s step
// that tick sits 8% of the track away, and the issue page shipped
// with `-5s` struck through by the word `error`. Found by looking at
// a screenshot; nothing in the tree could have said so.
//
// This checks the rule the fix installed: a tick prints its label
// only when there are at least `EVENT_LABEL_PX` between it and the
// event line. Not the constant's value — the property, at the widths
// and windows the page actually renders at.
import { tickLabelFits } from '../src/components/TimelineStrip.tsx';

const RESERVE = 96;
let bad = 0;
const check = (cond, msg) => {
  if (!cond) {
    console.error(`✗ ${msg}`);
    bad += 1;
  }
};

// The exact case that shipped broken.
check(!tickLabelFits(-5, 60, 800), '-5s prints beside `error 触发` on an 800px track');
check(tickLabelFits(-10, 60, 800), '-10s is dropped though it clears the label');

// Before the ResizeObserver has measured, assume the narrow pane: a
// missing tick reads as spacing, an overlap reads as broken.
check(!tickLabelFits(-5, 60, 0), 'an unmeasured track prints labels it cannot place');
check(tickLabelFits(0, 60, 0), 'the event label itself was suppressed');

// The property across the widths and windows the page renders at.
for (const trackW of [320, 500, 800, 1200, 1600]) {
  for (const spanS of [5, 15, 60, 120, 300, 600]) {
    for (const step of [1, 2, 5, 10, 15, 30, 60, 120]) {
      for (let sec = -spanS; sec < 0; sec += step) {
        if (!tickLabelFits(sec, spanS, trackW)) continue;
        const gapPx = ((-sec / spanS) * ((100 - 5) / 100)) * trackW;
        check(
          gapPx >= RESERVE,
          `${sec}s prints ${Math.round(gapPx)}px from the event line ` +
            `(span ${spanS}s, track ${trackW}px) — under the ${RESERVE}px reserve`,
        );
        if (bad > 3) break;
      }
    }
  }
}

if (bad > 0) process.exit(1);
console.log('✓ no tick label prints inside the event label’s reserve');
