import { describe, expect, test } from 'bun:test'

import plugin, {
  SentoriErrorBoundary,
  addBreadcrumb,
  captureException,
  sentori,
} from '../index.js'
import { setupTraceNavigation } from '../router.js'

describe('@goliapkg/sentori-vue exports', () => {
  test('default export is a Vue plugin with an install function', () => {
    expect(typeof plugin.install).toBe('function')
    expect(plugin).toBe(sentori)
  })

  test('SentoriErrorBoundary is a defineComponent', () => {
    expect(SentoriErrorBoundary).toBeDefined()
    expect(typeof SentoriErrorBoundary).toBe('object')
  })

  test('Vue Router helper is exported', () => {
    expect(typeof setupTraceNavigation).toBe('function')
  })

  test('re-exports common SDK helpers', () => {
    expect(typeof captureException).toBe('function')
    expect(typeof addBreadcrumb).toBe('function')
  })
})
