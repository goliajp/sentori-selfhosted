// How a crash-free rate is allowed to be written down.
//
// Two ways a percentage lies, and the card hit both:
//
//   · too many digits. 90.00% off ten sessions claims a precision the
//     data does not carry — one crash either way moves it ten points,
//     and the second decimal is arithmetic rather than measurement.
//   · rounding up to 100. A release with 900 sessions and 3 crashes is
//     99.67%, and `toFixed(0)` renders that as `100%` — which says the
//     release crashed for nobody, on a screen whose whole job is to say
//     otherwise. Cutting precision must never cost the fact that it
//     happened at all.
//
// So: precision follows the sample size, and a rate with at least one
// crash behind it is floored rather than rounded, which keeps it below
// 100 at every precision.

/** Digits a sample size can support. Under a thousand, one crash moves
 *  the first decimal, so the second is noise. */
const digitsFor = (samples: number): number => (samples < 1000 ? 1 : 2);

/**
 * `pct` as text, without the `%`.
 *
 * `crashed` is what makes this different from `toFixed`: it is the
 * evidence that 100 is the wrong answer however few digits are asked
 * for.
 */
export const formatCrashFree = (pct: number, samples: number, crashed: number): string => {
  const digits = digitsFor(samples);
  if (crashed <= 0) return pct.toFixed(digits);
  const scale = 10 ** digits;
  return (Math.floor(pct * scale) / scale).toFixed(digits);
};

/** One bucket of the window, as the crash-free endpoint answers it. */
export type TrendPoint = {
  at: string;
  crashFreeSessions: null | number;
  crashedSessions: number;
  sessions: number;
};

/**
 * The y-range a trend line should be drawn over.
 *
 * Not 0–100. Crash-free rates live in the top percent or two, and a
 * line drawn from zero is a flat line for every project that is not on
 * fire — it shows nothing, which is the same as not drawing it. So the
 * floor follows the worst bucket, and a little below it so that bucket
 * is a point on the chart rather than a point on the axis.
 *
 * The floor is clamped to at most 99 so a healthy window still has
 * visible room: without that, a window whose worst bucket is 99.9%
 * would magnify a tenth of a percent into the full height of the card
 * and read as a cliff.
 */
export const trendDomain = (points: TrendPoint[]): [number, number] => {
  const values = points
    .map((p) => p.crashFreeSessions)
    .filter((v): v is number => typeof v === 'number');
  if (values.length === 0) return [99, 100];
  const worst = Math.min(...values);
  return [Math.min(99, Math.floor(worst) - 1), 100];
};
