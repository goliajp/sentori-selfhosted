import { useEffect, useId, useRef, type ReactNode } from "react";
import { X } from "lucide-react";
import { Button } from "./controls";
export function Dialog({
  open,
  onClose,
  title,
  children,
  footer,
  drawer = false,
  active = true,
  closeLabel,
}: {
  open: boolean;
  onClose: () => void;
  title: string;
  children: ReactNode;
  footer?: ReactNode;
  drawer?: boolean;
  active?: boolean;
  closeLabel: string;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const id = useId();
  const controlledCloses = useRef(0);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    if (open && active && !el.open) {
      el.showModal();
      el.querySelector<HTMLElement>("[data-autofocus], [autofocus]")?.focus();
    }
    if ((!open || !active) && el.open) {
      // The queued native event may arrive after this dialog is active again.
      controlledCloses.current += 1;
      el.close();
    }
  }, [open, active]);
  return (
    <dialog
      ref={ref}
      aria-labelledby={id}
      className={`native-dialog ${drawer ? "drawer" : ""}`}
      onCancel={event => { if (event.target !== event.currentTarget) return; event.preventDefault(); event.stopPropagation(); onClose(); }}
      onClose={event => {
        if (event.target !== event.currentTarget) return;
        if (controlledCloses.current > 0) { controlledCloses.current -= 1; return; }
        if (open && active) onClose();
      }}
    >
      <div className="dialog-head">
        <h2 id={id}>{title}</h2>
        <Button
          variant="ghost"
          small
          className="button-icon"
          onClick={onClose}
          aria-label={closeLabel}
        >
          <X size={16} />
        </Button>
      </div>
      <div className="dialog-content">{children}</div>
      {footer && <div className="dialog-footer">{footer}</div>}
    </dialog>
  );
}