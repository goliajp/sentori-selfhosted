import { fireEvent, render, screen } from '@testing-library/react'
import { describe, expect, test } from 'bun:test'
import { useState } from 'react'

import { SentoriErrorBoundary } from '../SentoriErrorBoundary.js'
import { SentoriProvider } from '../SentoriProvider.js'

const PROVIDER_PROPS = {
  config: {
    environment: 'test',
    ingestUrl: 'http://localhost:0',
    release: 'test@0.0.0',
    token: 'st_pk_testtesttesttesttesttesttest',
  },
}

const Boom = (): never => {
  throw new Error('boom-from-render')
}

// React logs an "uncaught error" to console.error when a boundary
// catches; silence that during render-throw tests so the test output
// stays readable.
function silenceConsoleErrorDuring<T>(fn: () => T): T {
  const original = console.error
  console.error = () => {}
  try {
    return fn()
  } finally {
    console.error = original
  }
}

describe('SentoriErrorBoundary', () => {
  test('renders children when nothing throws', () => {
    render(
      <SentoriProvider {...PROVIDER_PROPS}>
        <SentoriErrorBoundary fallback={() => <div>fallback</div>}>
          <div>ok</div>
        </SentoriErrorBoundary>
      </SentoriProvider>,
    )
    expect(screen.getByText('ok')).toBeDefined()
  })

  test('renders fallback render-prop when child throws', () => {
    silenceConsoleErrorDuring(() => {
      render(
        <SentoriProvider {...PROVIDER_PROPS}>
          <SentoriErrorBoundary
            fallback={({ error }) => <div>caught: {error.message}</div>}
          >
            <Boom />
          </SentoriErrorBoundary>
        </SentoriProvider>,
      )
    })
    expect(screen.getByText('caught: boom-from-render')).toBeDefined()
  })

  test('accepts a ReactNode fallback (no render-prop)', () => {
    silenceConsoleErrorDuring(() => {
      render(
        <SentoriProvider {...PROVIDER_PROPS}>
          <SentoriErrorBoundary fallback={<div>static-fallback</div>}>
            <Boom />
          </SentoriErrorBoundary>
        </SentoriProvider>,
      )
    })
    expect(screen.getByText('static-fallback')).toBeDefined()
  })

  test('reset() callback clears the caught error', () => {
    // A child that throws on the first render and renders cleanly
    // once a flag flips. Pressing the fallback's Retry button calls
    // reset() AND flips the flag, so the boundary re-renders
    // children without a re-throw.
    function FlakyChild({ shouldThrow }: { shouldThrow: boolean }) {
      if (shouldThrow) throw new Error('flaky-boom')
      return <div>recovered</div>
    }

    function Harness() {
      const [throws, setThrows] = useState(true)
      return (
        <SentoriErrorBoundary
          fallback={({ reset }) => (
            <button
              onClick={() => {
                setThrows(false)
                reset()
              }}
              type="button"
            >
              Retry
            </button>
          )}
        >
          <FlakyChild shouldThrow={throws} />
        </SentoriErrorBoundary>
      )
    }

    silenceConsoleErrorDuring(() => {
      render(
        <SentoriProvider {...PROVIDER_PROPS}>
          <Harness />
        </SentoriProvider>,
      )
    })
    expect(screen.getByText('Retry')).toBeDefined()
    silenceConsoleErrorDuring(() => fireEvent.click(screen.getByText('Retry')))
    expect(screen.getByText('recovered')).toBeDefined()
  })

  test('resetKeys change clears the caught error automatically', () => {
    function FlakyChild({ shouldThrow }: { shouldThrow: boolean }) {
      if (shouldThrow) throw new Error('flaky-boom')
      return <div>recovered-by-keys</div>
    }

    function Harness() {
      const [keyA, setKeyA] = useState('one')
      const [throws, setThrows] = useState(true)
      return (
        <>
          <button
            onClick={() => {
              setThrows(false)
              setKeyA('two')
            }}
            type="button"
          >
            Switch
          </button>
          <SentoriErrorBoundary
            fallback={<div>error-state</div>}
            resetKeys={[keyA]}
          >
            <FlakyChild shouldThrow={throws} />
          </SentoriErrorBoundary>
        </>
      )
    }

    silenceConsoleErrorDuring(() => {
      render(
        <SentoriProvider {...PROVIDER_PROPS}>
          <Harness />
        </SentoriProvider>,
      )
    })
    expect(screen.getByText('error-state')).toBeDefined()
    silenceConsoleErrorDuring(() => fireEvent.click(screen.getByText('Switch')))
    expect(screen.getByText('recovered-by-keys')).toBeDefined()
  })

  test('onError receives the error and React ErrorInfo', () => {
    let captured: { error: Error | null; info: unknown } = { error: null, info: null }

    silenceConsoleErrorDuring(() => {
      render(
        <SentoriProvider {...PROVIDER_PROPS}>
          <SentoriErrorBoundary
            fallback={<div>fb</div>}
            onError={(error, info) => {
              captured = { error, info }
            }}
          >
            <Boom />
          </SentoriErrorBoundary>
        </SentoriProvider>,
      )
    })

    expect(captured.error?.message).toBe('boom-from-render')
    expect(captured.info).toBeDefined()
    // React 19's ErrorInfo at minimum has componentStack.
    expect((captured.info as { componentStack?: string }).componentStack).toBeDefined()
  })

  test('inner boundary catches without bubbling to outer', () => {
    let outerSawError = false

    silenceConsoleErrorDuring(() => {
      render(
        <SentoriProvider {...PROVIDER_PROPS}>
          <SentoriErrorBoundary
            fallback={<div>outer-fb</div>}
            onError={() => {
              outerSawError = true
            }}
          >
            <SentoriErrorBoundary fallback={<div>inner-fb</div>}>
              <Boom />
            </SentoriErrorBoundary>
            <div>sibling-stays</div>
          </SentoriErrorBoundary>
        </SentoriProvider>,
      )
    })

    expect(screen.getByText('inner-fb')).toBeDefined()
    // Sibling next to the inner boundary keeps rendering — the outer
    // boundary's subtree is intact because the inner caught.
    expect(screen.getByText('sibling-stays')).toBeDefined()
    expect(outerSawError).toBe(false)
  })
})
