export const fileIdentity = (file: File) => `${file.name}\u0000${file.size}\u0000${file.lastModified}\u0000${file.type}`;
export function acceptsFile(file: Pick<File, 'name' | 'type'>, accept = '') {
  const rules = accept.toLowerCase().split(',').map(s => s.trim()).filter(Boolean);
  return !rules.length || rules.some(rule => rule.startsWith('.') ? file.name.toLowerCase().endsWith(rule) : rule.endsWith('/*') ? file.type.toLowerCase().startsWith(rule.slice(0, -1)) : file.type.toLowerCase() === rule);
}
export function selectFiles(current: readonly File[], incoming: readonly File[], { accept = '', multiple = true, maxFiles = 10, maxFileSize = Infinity }: { accept?: string; multiple?: boolean; maxFiles?: number; maxFileSize?: number } = {}) {
  const next = multiple ? [...current] : [], added: File[] = [], rejected: { file: File; reason: 'type' | 'size' | 'count' | 'duplicate' }[] = [];
  for (const file of incoming) {
    const reason = !acceptsFile(file, accept) ? 'type' : file.size > maxFileSize ? 'size' : next.some(item => fileIdentity(item) === fileIdentity(file)) ? 'duplicate' : next.length >= (multiple ? maxFiles : 1) ? 'count' : undefined;
    if (reason) rejected.push({ file, reason }); else { next.push(file); added.push(file); }
  }
  // An invalid replacement must not discard a previously selected single file.
  return { files: !multiple && !added.length ? [...current] : next, added, rejected };
}
