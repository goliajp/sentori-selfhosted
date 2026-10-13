import { useLayoutEffect, useRef, type InputHTMLAttributes, type ReactNode } from 'react';

type CheckboxInputProps = Omit<InputHTMLAttributes<HTMLInputElement>, 'type' | 'checked'> & {
  checked: boolean | 'mixed';
};

/** Keep native form/keyboard semantics, with one shared checked/mixed style. */
export function CheckboxInput({ checked, ...props }: CheckboxInputProps) {
  const input = useRef<HTMLInputElement>(null);
  useLayoutEffect(() => {
    if (input.current) input.current.indeterminate = checked === 'mixed';
  }, [checked]);
  return <input {...props} ref={input} type="checkbox" checked={checked === true} />;
}

export function Checkbox({ label, onChange, className = '', ...props }: Omit<CheckboxInputProps, 'onChange'> & {
  label: ReactNode;
  onChange: (checked: boolean) => void;
}) {
  return <label className={`choice-control ${className}`}>
    <CheckboxInput {...props} onChange={event => onChange(event.target.checked)} />
    <span className="choice-label">{label}</span>
  </label>;
}

export function Radio({ label, className = '', ...props }: Omit<InputHTMLAttributes<HTMLInputElement>, 'type'> & { label: ReactNode }) {
  return <label className={`choice-control ${className}`}>
    <input {...props} type="radio" />
    <span className="choice-label">{label}</span>
  </label>;
}
