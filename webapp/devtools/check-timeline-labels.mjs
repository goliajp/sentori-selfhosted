// Numeric ruler labels stay clear of the zero endpoint at every panel width.
import { tickLabelFits } from '../src/components/TimelineStrip.tsx';

const RESERVE = 64;
let bad = 0;
const check = (cond, msg) => {
  if (!cond) {
    console.error(`✗ ${msg}`);
    bad += 1;
  }
};

// The exact case that shipped broken.
check(tickLabelFits(-5, 60, 800), '-5s is hidden despite room beside the numeric 0s label');
check(!tickLabelFits(-5, 60, 500), '-5s crowds the zero endpoint on a 500px track');
check(tickLabelFits(-60, 60, 80), 'the start endpoint must remain visible in a narrow pane');
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
        if (sec === -spanS || !tickLabelFits(sec, spanS, trackW)) continue;
        const gapPx = (-sec / spanS) * trackW;
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
console.log('✓ no tick label prints inside the zero endpoint’s reserve');
