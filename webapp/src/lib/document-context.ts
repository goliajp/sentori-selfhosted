import { createContext } from 'react';
/** Stable readers remain correct when React pauses an inactive document. */
export const DocumentRuntimeContext = createContext<null | {
  isActive: () => boolean;
  search: () => string;
}>(null);
