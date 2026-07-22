import React, { Component, type ErrorInfo, type ReactNode } from 'react';

import { captureError } from './capture';

export type ErrorBoundaryProps = {
  fallback?: ReactNode | ((error: Error, reset: () => void) => ReactNode);
  children: ReactNode;
};

type State = { error: Error | null };

export class ErrorBoundary extends Component<ErrorBoundaryProps, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, _info: ErrorInfo): void {
    captureError(error);
  }

  reset = (): void => {
    this.setState({ error: null });
  };

  render(): ReactNode {
    const { error } = this.state;
    const { fallback, children } = this.props;
    if (error) {
      if (typeof fallback === 'function') {
        return fallback(error, this.reset);
      }
      return fallback ?? null;
    }
    return children;
  }
}
