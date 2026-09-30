// The Sentori mark, inline.
//
// 鳥居 — the watching gate: a threshold marker, and what passes
// through it is seen. Straight cuts and miter joins put it in the
// GOLIA Lab family (the plate variant — blue square, white gate —
// is the favicon and app icon; see /brand).
//
// Inline rather than an <img> on purpose: in product chrome the mark
// must take the surrounding ink (`currentColor`) instead of fighting
// the UI accent. The plated, always-blue form belongs to icons.

export function SentoriMark({ className = '' }: { className?: string }) {
  return (
    <svg
      viewBox="0 0 32 32"
      aria-hidden
      className={className}
      fill="none"
      stroke="currentColor"
      strokeWidth={2.7}
      strokeLinejoin="miter"
      strokeLinecap="butt"
    >
      <path d="M6 10 H26 M9 15 H23 M12 10 L10.5 26 M20 10 L21.5 26" />
    </svg>
  );
}
