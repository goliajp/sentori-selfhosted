import type { ReactNode } from 'react';
import { Activity, Inbox, ListTodo } from 'lucide-react';

export function WorkbenchEmpty({kind='activity',title,description,action}: {kind?:'activity'|'tasks'|'inbox';title:string;description?:string;action?:ReactNode}) {
  const Icon = kind==='tasks'?ListTodo:kind==='inbox'?Inbox:Activity;
  return <div className="workbench-empty">
    <div className="workbench-empty-icon" aria-hidden><Icon size={28} strokeWidth={1.5}/></div>
    <h2>{title}</h2>{description&&<p>{description}</p>}{action&&<div className="workbench-empty-actions">{action}</div>}
  </div>;
}
