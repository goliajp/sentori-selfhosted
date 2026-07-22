import { Component, type ErrorInfo, type ReactNode } from 'react'

import { useSentoriCtx } from './SentoriProvider.js'

type FallbackRender = (props: { error: Error; reset: () => void }) => ReactNode

type Props = {
  children: ReactNode
  /**
   * Rendered after an error is caught. Either a plain ReactNode
   * (most common — a static error screen) or a render-prop that
   * receives the error and a `reset` callback so the fallback can
   * offer a retry button.
   */
  fallback: FallbackRender | ReactNode
  /** Optional additional logging hook. Runs after Sentori capture. */
  onError?: (error: Error, info: ErrorInfo) => void
  /**
   * Shallow-compared on update. Any change resets the boundary,
   * letting parents recover from a caught error by passing fresh
   * keys (e.g. a route path, a query key, a user id).
   */
  resetKeys?: unknown[]
}

type State = { error: Error | null }

export function SentoriErrorBoundary(props: Props) {
  const { captureError } = useSentoriCtx()
  return (
    <SentoriErrorBoundaryInner
      {...props}
      capture={(err, info) => {
        captureError(err, { tags: { source: 'react.errorBoundary' } })
        props.onError?.(err, info)
      }}
    />
  )
}

class SentoriErrorBoundaryInner extends Component<
  Props & { capture: (e: Error, info: ErrorInfo) => void },
  State
> {
  state: State = { error: null }

  static getDerivedStateFromError(error: Error): State {
    return { error }
  }

  componentDidCatch(error: Error, info: ErrorInfo): void {
    this.props.capture(error, info)
  }

  componentDidUpdate(prev: Readonly<Props>): void {
    if (this.state.error && resetKeysChanged(prev.resetKeys, this.props.resetKeys)) {
      this.setState({ error: null })
    }
  }

  reset = (): void => this.setState({ error: null })

  render(): ReactNode {
    const { error } = this.state
    if (error) {
      const { fallback } = this.props
      return typeof fallback === 'function'
        ? (fallback as FallbackRender)({ error, reset: this.reset })
        : fallback
    }
    return this.props.children
  }
}

function resetKeysChanged(prev?: unknown[], next?: unknown[]): boolean {
  if (prev === next) return false
  if (!prev || !next) return prev !== next
  if (prev.length !== next.length) return true
  for (let i = 0; i < prev.length; i++) {
    if (!Object.is(prev[i], next[i])) return true
  }
  return false
}
