/** Artifact validation is not platform coverage or a claim of build matching. */
export type LightState = 'valid' | 'partial' | 'invalid' | 'absent' | 'unchecked';

const artifactKinds = ['sourcemap', 'dsym', 'proguard', 'srcbundle'] as const;

/** Native and JavaScript builds can share a platform; report actual inventory
 * rather than inferring required files from an iOS/Android label. */
export function uploadedArtifactKinds(release: {
  artifacts?: { kind: string }[] | null;
  artifactSummary?: { kind: string; count: number }[];
}) {
  return artifactKinds.filter(kind => release.artifacts != null
    ? release.artifacts.some(file => file.kind === kind)
    : release.artifactSummary?.some(summary => summary.kind === kind && summary.count > 0));
}

export function lightState({ count, usable, broken, unchecked }: {
  count?: number; usable?: boolean; broken?: boolean; unchecked?: boolean;
}): LightState {
  if (count === undefined) return 'unchecked';
  if (count === 0) return 'absent';
  if (broken) return usable || unchecked ? 'partial' : 'invalid';
  if (unchecked || !usable) return 'unchecked';
  return 'valid';
}

export function lightColour(state: LightState): string {
  switch (state) {
    case 'valid': return 'var(--s-kind-probe)';
    case 'partial': return 'var(--s-kind-warn)';
    case 'invalid': return 'var(--s-kind-error)';
    default: return 'color-mix(in srgb, var(--sn-fg-muted) 30%, transparent)';
  }
}
