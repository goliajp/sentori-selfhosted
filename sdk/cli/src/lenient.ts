// The iron rule reaches build-time (design.md §6 dim 3): an upload
// failure must never break the customer's build or release. Every
// upload-family command runs through this wrapper — on failure it
// prints a friendly, actionable note and exits 0. `--strict` opts
// into a real non-zero exit for pipelines that want the hard
// guarantee.

export type LenientOutcome = {
  /** What failed, one line. */
  failure: string
  /** What it means for the customer, one or two lines. */
  impact: string
  /** A copy-pasteable retry command. */
  retry: string
}

export const isStrict = (argv: string[]): boolean => argv.includes('--strict')

/** Strip --strict before handing argv to parseArgs. */
export const stripStrict = (argv: string[]): string[] =>
  argv.filter((a) => a !== '--strict')

export const lenientFail = (strict: boolean, o: LenientOutcome): number => {
  console.error(`[sentori] ⚠ ${o.failure}`)
  if (!strict) {
    console.error('[sentori]   Your build is NOT blocked by this.')
  }
  console.error(`[sentori]   Impact: ${o.impact}`)
  console.error('[sentori]   Retry anytime:')
  console.error(`[sentori]     ${o.retry}`)
  console.error(
    '[sentori]   Events received in the meantime are re-symbolicated automatically after upload.',
  )
  if (strict) {
    console.error('[sentori]   (--strict: exiting non-zero)')
    return 1
  }
  return 0
}
