import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, test } from 'bun:test'

import { SentoriProvider } from '../SentoriProvider.js'
import { SentoriSuspense } from '../SentoriSuspense.js'

const PROVIDER_PROPS = {
  config: {
    environment: 'test',
    ingestUrl: 'http://localhost:0',
    release: 'test@0.0.0',
    token: 'st_pk_testtesttesttesttesttesttest',
  },
}

function silenceConsoleErrorDuring<T>(fn: () => T): T {
  const original = console.error
  console.error = () => {}
  try {
    return fn()
  } finally {
    console.error = original
  }
}

describe('SentoriSuspense', () => {
  afterEach(() => cleanup())

  test('renders children when nothing throws', () => {
    render(
      <SentoriProvider {...PROVIDER_PROPS}>
        <SentoriSuspense fallback={<div>loading</div>}>
          <div>child-ok</div>
        </SentoriSuspense>
      </SentoriProvider>,
    )
    expect(screen.getByText('child-ok')).toBeDefined()
  })

  test('synchronously thrown error inside is caught and renders errorFallback', () => {
    function Boom(): never {
      throw new Error('sync-boom')
    }

    silenceConsoleErrorDuring(() => {
      render(
        <SentoriProvider {...PROVIDER_PROPS}>
          <SentoriSuspense
            errorFallback={<div>error-fallback</div>}
            fallback={<div>loading</div>}
          >
            <Boom />
          </SentoriSuspense>
        </SentoriProvider>,
      )
    })

    expect(screen.getByText('error-fallback')).toBeDefined()
  })

  test('errorFallback defaults to fallback when not provided', () => {
    function Boom(): never {
      throw new Error('sync-boom-default')
    }

    silenceConsoleErrorDuring(() => {
      render(
        <SentoriProvider {...PROVIDER_PROPS}>
          <SentoriSuspense fallback={<div>shared-fallback</div>}>
            <Boom />
          </SentoriSuspense>
        </SentoriProvider>,
      )
    })

    expect(screen.getByText('shared-fallback')).toBeDefined()
  })
})
