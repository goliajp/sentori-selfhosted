import type { ReactNode } from "react";
import { useSite } from "./runtime";
export function AuthLayout({ art, title, description, children }: { art: ReactNode; title: string; description: string; children: ReactNode }) {
  const { t } = useSite();
  return <div className="auth-layout"><div className="auth-art"><div className="auth-art-copy"><span className="eyebrow">{t("site.authEyebrow")}</span><h2>{title}</h2><p>{description}</p></div><div className="auth-art-scene">{art}</div><div className="auth-art-footnote">{t("site.authFootnote")}</div></div>{children}</div>;
}
