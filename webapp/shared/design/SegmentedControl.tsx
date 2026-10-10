import { useId } from "react";
import { FieldFeedback } from "./FieldFeedback";
import { selectionOptions, type SelectionOptionInput } from "./selection-model";
type GroupProps = {
  label: string;
  options: readonly SelectionOptionInput[];
  hint?: string;
  error?: string;
  reserveMessage?: boolean;
  name?: string;
  disabled?: boolean;
  required?: boolean;
  className?: string;
};
type SingleGroupProps = GroupProps & {
  value: string;
  onChange: (value: string) => void;
};
function GroupHelp({
  id,
  hint,
  error,
  reserveSpace,
}: {
  id: string;
  hint?: string;
  error?: string;
  reserveSpace?: boolean;
}) {
  return <FieldFeedback id={id} hint={hint} error={error} reserveSpace={reserveSpace} />;
}
function describedBy(id: string, hint?: string, error?: string) {
  return (
    error ? `${id}-error` : hint ? `${id}-hint` : undefined
  );
}

export function SegmentedControl({
  label,
  options,
  value,
  onChange,
  hint,
  error,
  reserveMessage = false,
  name,
  disabled,
  required,
  className = "",
  hideLabel = false,
}: SingleGroupProps & { hideLabel?: boolean }) {
  const id = useId();
  return (
    <fieldset
      className={`selection-group segmented-field ${className}`}
      disabled={disabled}
      aria-describedby={describedBy(id, hint, error)}
      aria-invalid={!!error || undefined}
    >
      <legend className={hideLabel ? "sr-only" : "field-label"}>{label}</legend>
      <div className="segmented-control">
        {selectionOptions(options).map((option) => (
          <label
            key={option.value}
            className={`segment ${value === option.value ? "is-selected" : ""}`}
            lang={option.lang}
          >
            <input
              className="sr-only"
              type="radio"
              name={name ?? id}
              value={option.value}
              checked={value === option.value}
              onChange={() => onChange(option.value)}
              required={required}
              disabled={option.disabled}
            />
            {option.icon && <span aria-hidden="true">{option.icon}</span>}
            <span className="control-label-text">{option.label}</span>
          </label>
        ))}
      </div>
      <GroupHelp id={id} hint={hint} error={error} reserveSpace={reserveMessage} />
    </fieldset>
  );
}

