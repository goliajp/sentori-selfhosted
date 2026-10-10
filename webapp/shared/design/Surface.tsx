import type { ReactNode } from "react";
export function Heading({
  title,
  description,
  action,
  level = 1,
}: {
  title: string;
  description?: string;
  action?: ReactNode;
  level?: 1 | 2;
}) {
  const Title = level === 1 ? "h1" : "h2";
  return (
    <header className="page-heading">
      <div>
        <Title>{title}</Title>
        {description && <p>{description}</p>}
      </div>
      {action}
    </header>
  );
}
export function Surface({
  title,
  action,
  footer,
  children,
  className = "",
  body = true,
  appearance = "outlined",
  dividers = "auto",
}: {
  title?: string;
  action?: ReactNode;
  footer?: ReactNode;
  children: ReactNode;
  className?: string;
  body?: boolean;
  appearance?: "outlined" | "tonal" | "plain";
  dividers?: "auto" | "always" | "none";
}) {
  return (
    <section className={`surface ${className}`} data-appearance={appearance} data-dividers={dividers}>
      {title && (
        <header className="surface-head">
          <h2>{title}</h2>
          {action}
        </header>
      )}
      <div className={body ? "surface-body" : undefined}>{children}</div>
      {footer && <footer className="surface-foot">{footer}</footer>}
    </section>
  );
}
