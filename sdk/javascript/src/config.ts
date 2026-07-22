import type { InitOptions } from './types.js'

let _cfg: InitOptions | null = null

export function setConfig(cfg: InitOptions): void {
  _cfg = cfg
}

export function getConfig(): InitOptions | null {
  return _cfg
}

export function isInitialized(): boolean {
  return _cfg !== null
}

export function __resetForTests(): void {
  _cfg = null
}
