import { Children, cloneElement, isValidElement, useId, type ButtonHTMLAttributes, type ComponentPropsWithRef, type TextareaHTMLAttributes, type ReactElement, type ReactNode } from "react";
import { Spinner } from "./Spinner";
import { FieldFeedback } from "./FieldFeedback";

/** Shared interaction primitive for buttons, selectable rows and chart controls.
 * Composed controls own their layout; focus, disabled and form semantics live here.
 */
export function Action({ onClick, className = '', ...props }: ComponentPropsWithRef<'button'>) {
  return <button type="button" {...props} className={`control-action ${className}`} onClick={event => {
    if (event.detail > 0) event.currentTarget.focus({ preventScroll: true });
    onClick?.(event);
  }} />;
}

export function Button({
  variant = "secondary",
  busy = false,
  small = false,
  size,
  children,
  className = "",
  disabled,
  onClick,
  ...props
}: ButtonHTMLAttributes<HTMLButtonElement> & {
  variant?: "primary" | "secondary" | "ghost" | "danger";
  busy?: boolean;
  small?: boolean;
  size?: 'small' | 'regular' | 'large';
}) {
  return (
    <Action
      {...props}
      onClick={onClick}
      disabled={disabled || busy}
      aria-busy={busy || undefined}
      className={`button button-${variant} ${size === 'large' ? 'button-large' : small || size === 'small' ? 'button-small' : ''} ${className}`}
    >
      {busy && (
        <Spinner
          size={16}
          className="button-spinner"
          aria-hidden="true"
        />
      )}
      <span className="button-content">
        {Children.map(children, child =>
          typeof child === "string" || typeof child === "number"
            ? <span className="button-label">{child}</span>
            : child,
        )}
      </span>
    </Action>
  );
}
export function Field(props: {
  label: ReactNode;
  hint?: ReactNode;
  error?: string;
  reserveMessage?: boolean;
  required?: boolean;
  children: ReactNode;
  className?: string;
}) {
  const {
  label,
  hint,
  error,
  children,
  className = "",
  required,
  reserveMessage = Object.hasOwn(props, "error"),
  } = props;
  const id = useId();
  const child = Children.only(children);
  const isRequired = required ?? (isValidElement(child) && !!(child.props as { required?: boolean }).required);
  return (
    <div className={`field ${className}`}>
      <label className="field-label" htmlFor={id}>
        {label}{isRequired && <span className="field-required" aria-hidden="true">*</span>}
      </label>
      {isValidElement(child)
        ? cloneElement(child as ReactElement<Record<string, unknown>>, {
            id,
            required: isRequired || undefined,
            "aria-describedby": error
              ? `${id}-error`
              : hint
                ? `${id}-hint`
                : undefined,
            "aria-invalid": error ? true : undefined,
          })
        : child}
      <FieldFeedback id={id} hint={hint} error={error} reserveSpace={reserveMessage} />
    </div>
  );
}
export function Input(props: ComponentPropsWithRef<'input'>) {
  return (
    <input
      {...props}
      data-autofocus={props.autoFocus || undefined}
      className={`control ${props.className ?? ""}`}
    />
  );
}
/** Native selection retains browser keyboard support within the shared control skin. */
export function NativeSelect({ className = '', ...props }: ComponentPropsWithRef<'select'>) {
  return <select {...props} className={`control native-select ${className}`} />;
}
/** Five text rows by default; a one-row textarea shares the input's text geometry. */
export function Textarea({ rows = 5, className = '', ...props }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea {...props} rows={rows} data-autofocus={props.autoFocus || undefined} className={`control ${className}`} />;
}
/** A row owns control sizing; small standalone actions join the row's rhythm. */
export function ControlRow({
  children,
  size = "regular",
  className = "",
}: {
  children: ReactNode;
  size?: "regular" | "small";
  className?: string;
}) {
  return (
    <div className={`control-row ${className}`} data-control-size={size}>
      {children}
    </div>
  );
}
export function Badge({
  children,
  tone = "neutral",
}: {
  children: ReactNode;
  tone?: "neutral" | "success" | "warning" | "danger" | "info" | "accent";
}) {
  return <span className={`badge badge-${tone}`}>{Children.map(children, child => typeof child === "string" || typeof child === "number" ? <span className="badge-label">{child}</span> : child)}</span>;
}
