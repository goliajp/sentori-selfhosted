import type { SVGProps } from 'react';

/** A fixed square viewport and track; only the arc rotates around the exact circle center. */
export function Spinner({ size = 16, className = '', ...props }: SVGProps<SVGSVGElement> & { size?: number }) {
  return <svg {...props} className={`spinner ${className}`} width={size} height={size} viewBox="0 0 24 24" fill="none" aria-hidden="true" focusable="false">
    <circle className="spinner-track" cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="2" opacity=".2" />
    <circle className="spinner-arc" cx="12" cy="12" r="9" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeDasharray="18 56.55" />
  </svg>;
}
