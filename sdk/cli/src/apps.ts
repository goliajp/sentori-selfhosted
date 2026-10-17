import type { ApiConfig } from './issue.js'

export type AppSummary = {
  id: string
  projectId: string
  name: string | null
  identifier: string
  platform: string
  kind: string
  events24h: number
  errors24h: number
  releaseCount: number
}

async function read<T>(config: ApiConfig, path: string): Promise<T> {
  const response = await fetch(`${config.apiUrl.replace(/\/+$/, '')}${path}`, {
    headers: { Authorization: `Bearer ${config.token}` },
    redirect: 'error',
    signal: AbortSignal.timeout(30_000),
  })
  if (!response.ok) throw new Error(`Sentori API returned ${response.status}`)
  return await response.json() as T
}

export function listApps(config: ApiConfig): Promise<{ projectId: string; apps: AppSummary[] }> {
  return read(config, '/api/apps')
}

export function showProject(config: ApiConfig): Promise<{ project: { id: string; name: string; createdAt: string } }> {
  return read(config, '/api/project')
}
