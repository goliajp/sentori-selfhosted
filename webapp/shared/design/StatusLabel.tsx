import type { HTMLAttributes, ReactNode } from 'react';

/** One centered text box for status marks, legends and short inline labels. */
export function StatusLabel({ marker, children, truncate = false, className = '', ...props }: HTMLAttributes<HTMLSpanElement> & { marker?: ReactNode; truncate?: boolean }) {
  return <span {...props} className={`status-label ${truncate ? 'status-label-truncate' : ''} ${className}`}>
    {marker && <span className="status-label-marker" aria-hidden="true">{marker}</span>}
    <span className="status-label-text">{children}</span>
  </span>;
}
