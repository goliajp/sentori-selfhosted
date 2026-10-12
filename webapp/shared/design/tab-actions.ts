export type TabCloseAction = 'close' | 'others' | 'left' | 'right' | 'all';
export function tabCloseTargets(keys: string[], key: string, action: TabCloseAction) {
  const index=keys.indexOf(key);
  if(index<0) return [];
  return action==='all'?keys:action==='close'?[key]:action==='others'?keys.filter(value=>value!==key):action==='left'?keys.slice(0,index):keys.slice(index+1);
}
