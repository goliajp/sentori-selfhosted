import { createContext, useContext } from 'react';
export type SiteRuntime = {
  t: (key: string, params?: Record<string,string|number>) => string;
  locale: 'zh' | 'ja' | 'en';
  number: (value: number, digits?: number) => string;
  href: (page: string, params?: Record<string, string>) => string;
  navigate: (page: string, params?: Record<string, string>) => void;
  image: (path: string) => string;
  path: string;
};
export const SiteContext = createContext<SiteRuntime | null>(null);
export function useSite() {
  const runtime = useContext(SiteContext);
  if (!runtime) throw new Error('Site components require a SiteContext');
  return runtime;
}
