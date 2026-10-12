import type { ReactNode } from "react";

export function WorkspaceLayout({ scope = "studio", workbench = false, sidebar, toolbar, children, className = "" }: {
  className?: string; scope?: "studio" | "website" | "product"; workbench?: boolean; sidebar: ReactNode; toolbar: ReactNode; children: ReactNode;
}) {
  const header = <header className="studio-topbar">{toolbar}</header>;
  return <div className={`studio-shell ${className}`} data-scope={scope} data-workbench={workbench}>
    {workbench && header}
    <aside className="studio-nav desktop">{sidebar}</aside>
    <div className="studio-content">{!workbench && header}{children}</div>
  </div>;
}

