import { useId, type ReactNode } from "react";
export function SectionLayout({ title, description, children }: { title: string; description?: string; children: ReactNode }) {
  const id = useId();
  return <section className="welcome-features" aria-labelledby={id}><h2 id={id}>{title}</h2>{description && <p className="muted text-sm mb-6">{description}</p>}{children}</section>;
}
