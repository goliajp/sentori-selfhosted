import type { ReactNode } from "react";
/** Validation replaces help in one reserved slot instead of moving the next control. */
export function FieldFeedback({ id, hint, error, reserveSpace = false }: {
  id: string; hint?: ReactNode; error?: string; reserveSpace?: boolean;
}) {
  if (!hint && !error && !reserveSpace) return null;
  return <div className="field-feedback" data-reserved={reserveSpace || undefined}>
    {error ? <p id={`${id}-error`} className="field-error" role="alert">{error}</p>
      : hint ? <p id={`${id}-hint`} className="field-note">{hint}</p> : null}
  </div>;
}
