import { createContext } from 'react';
/** Stable readers remain correct when React pauses an inactive document. */
export const DocumentRuntimeContext = createContext<null | {
  isActive: () => boolean;
  search: () => string;
}>(null);

/** A document may project its selected evidence into the shared inspector. */
export const DocumentInspectionContext = createContext<HTMLElement | null>(null);
