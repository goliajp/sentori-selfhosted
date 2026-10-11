export type EventTag = {source: 'context' | 'tags'; key: string; value: string};
export type EventFilters = {appId?: string; platform?: string; environment?: string; release?: string; tags?: EventTag[]};

export function eventFilterParams(filters: EventFilters): URLSearchParams {
  const params = new URLSearchParams();
  if (filters.appId) params.set('app_id', filters.appId);
  if (filters.platform) params.set('platform', filters.platform);
  if (filters.environment) params.set('environment', filters.environment);
  if (filters.release) params.set('release', filters.release);
  if (filters.tags?.length) params.set('tags', JSON.stringify(filters.tags));
  return params;
}

export function readEventFilters(params: URLSearchParams): EventFilters {
  const tags: EventTag[] = JSON.parse(params.get('tags') ?? '[]');
  if (!Array.isArray(tags) || tags.length > 12 || tags.some(tag => !tag || !['context','tags'].includes(tag.source) || typeof tag.key !== 'string' || !tag.key || typeof tag.value !== 'string')) throw new Error('invalid_event_tags');
  const key = params.get('ck') ?? params.get('dimension');
  const value = params.get('cv') ?? params.get('context');
  if (key && value !== null) tags.push({source:'context',key,value});
  return {appId:params.get('app') || undefined, platform:params.get('platform') || params.get('appType') || undefined,
    environment:params.get('env') || undefined, release:params.get('rel') || params.get('release') || undefined, tags};
}
