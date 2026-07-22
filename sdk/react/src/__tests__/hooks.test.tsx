import { render, screen } from '@testing-library/react'
import { describe, expect, test } from 'bun:test'

import { SentoriProvider } from '../SentoriProvider.js'
import { useSentori } from '../hooks.js'

const PROVIDER_PROPS = {
  config: {
    environment: 'test',
    ingestUrl: 'http://localhost:0',
    release: 'test@0.0.0',
    token: 'st_pk_testtesttesttesttesttesttest',
  },
}

describe('useSentori', () => {
  test('returns capture / setUser / addBreadcrumb', () => {
    const Probe = () => {
      const ctx = useSentori()
      return (
        <div>
          init={String(ctx.initialised)} hasCapture=
          {String(typeof ctx.captureError === 'function')}
        </div>
      )
    }
    render(
      <SentoriProvider {...PROVIDER_PROPS}>
        <Probe />
      </SentoriProvider>,
    )
    expect(screen.getByText(/init=true hasCapture=true/)).toBeDefined()
  })

  test('throws when used outside provider', () => {
    const Probe = () => {
      useSentori()
      return null
    }
    const original = console.error
    console.error = () => {}
    try {
      expect(() => render(<Probe />)).toThrow(/SentoriProvider/)
    } finally {
      console.error = original
    }
  })
})
