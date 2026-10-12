import { createContext, useContext, type ReactNode } from 'react';
import { DocumentRuntimeContext } from './document-context';
import type { Me, Project } from './api';
export type ProjectDestination = { pathname: string; params?: Record<string, string> };
export type Shell = {
  me: Me; projects: Project[]; reloadProjects: () => Promise<void>;
  activeProject: Project | null; setActiveProjectId: (id: string, destination?: ProjectDestination) => void;
};
export const ShellContext = createContext<Shell | null>(null);
export function useShell(): Shell {
  const shell = useContext(ShellContext);
  if (!shell) throw new Error('useShell outside <App>');
  return shell;
}
/** Inactive documents keep their original project while the outer shell changes. */
export function DocumentProject({ projectId, children }: { projectId: string; children: ReactNode }) {
  const shell = useShell(), runtime=useContext(DocumentRuntimeContext);
  return <ShellContext.Provider value={{ ...shell, setActiveProjectId:(id,destination)=>{if(!runtime || runtime.isActive())shell.setActiveProjectId(id,destination);}, activeProject: shell.projects.find(project => project.id === projectId) ?? null }}>{children}</ShellContext.Provider>;
}
