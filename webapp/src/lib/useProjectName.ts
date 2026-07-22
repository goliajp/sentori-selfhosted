// The name of the project you are looking at.
//
// Pages under /projects/:id have the id and nothing else, and several
// of them were putting `Project 019e358a…` in the page subtitle. A
// truncated uuid is how the row is stored, not how anyone thinks about
// their app — the person reading it knows it as "insight-mobile".
//
// Falls back to the short id while the list is in flight, so the header
// never jumps from empty to text.

import { useEffect, useState } from 'react';

import { api } from './api';

export function useProjectName(projectId: string | undefined): string {
  const [name, setName] = useState<string | null>(null);

  useEffect(() => {
    if (!projectId) return;
    let live = true;
    api
      .listProjects()
      .then(ps => {
        if (live) setName(ps.find(p => p.id === projectId)?.name ?? null);
      })
      .catch(() => {
        // A missing name is not worth an error banner over the issue
        // list; the short id below still identifies the project.
      });
    return () => {
      live = false;
    };
  }, [projectId]);

  return name ?? `${projectId?.slice(0, 8) ?? ''}…`;
}
