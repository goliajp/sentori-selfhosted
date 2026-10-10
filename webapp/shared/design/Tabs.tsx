import { useId, type ReactNode } from "react";
export function Tabs({
  items,
  value,
  onChange,
  children,
  variant = "segmented",
  label,
}: {
  items: readonly (readonly [string, string])[];
  value: string;
  onChange: (v: string) => void;
  children?: ReactNode;
  variant?: "segmented" | "line";
  label?: string;
}) {
  const id = useId();
  return (
    <>
      <div
        role="tablist"
        className={`tabs tabs-${variant}`}
        aria-label={label}
        onKeyDown={(e) => {
          let i = items.findIndex((x) => x[0] === value);
          if (e.key === "ArrowRight") i = (i + 1) % items.length;
          else if (e.key === "ArrowLeft")
            i = (i + items.length - 1) % items.length;
          else if (e.key === "Home") i = 0;
          else if (e.key === "End") i = items.length - 1;
          else return;
          e.preventDefault();
          onChange(items[i][0]);
          document.getElementById(`${id}-${items[i][0]}`)?.focus();
        }}
      >
        {items.map(([key, label]) => (
          <button
            type="button"
            role="tab"
            id={`${id}-${key}`}
            key={key}
            aria-controls={`${id}-panel`}
            aria-selected={key === value}
            tabIndex={key === value ? 0 : -1}
            onClick={() => onChange(key)}
          >
            {label}
          </button>
        ))}
      </div>
      <div
        role="tabpanel"
        id={`${id}-panel`}
        aria-labelledby={`${id}-${value}`}
        tabIndex={0}
      >
        {children}
      </div>
    </>
  );
}
