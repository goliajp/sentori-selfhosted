export type ReplayViewport = { width: number; height: number };
/** Uniform contain-fit keeps the captured coordinate system, including pointer positions. */
export function fitReplayViewport(stage: number, viewport: ReplayViewport) {
  if (![stage, viewport.width, viewport.height].every(Number.isFinite) || stage <= 0 || viewport.width <= 0 || viewport.height <= 0) return { width: 0, height: 0, scale: 0 };
  const scale = Math.min(stage / viewport.width, stage / viewport.height);
  return { width: viewport.width * scale, height: viewport.height * scale, scale };
}
