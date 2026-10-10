import { siteMessages } from '../../shared/site/messages';
import { useLocale } from './index';
/** The frozen component catalogue is shared by public and workspace controls. */
export function useSharedT() {
  const locale = useLocale();
  return (key:string,params?:Record<string,string|number>) => {
    const row=siteMessages[key as keyof typeof siteMessages];
    if(!row)throw new Error(`Missing shared message: ${key}`);
    return row[{zh:0,ja:1,en:2}[locale]].replace(/\{(\w+)\}/g,(match,name:string)=>params?.[name]==null?match:String(params[name]));
  };
}
