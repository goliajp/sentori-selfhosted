import { jsx as _jsx } from "react/jsx-runtime";
import { Component } from 'react';
import { useSentoriCtx } from './SentoriProvider.js';
export function SentoriErrorBoundary(props) {
    const { captureError } = useSentoriCtx();
    return (_jsx(SentoriErrorBoundaryInner, { ...props, capture: (err, info) => {
            captureError(err, { tags: { source: 'react.errorBoundary' } });
            props.onError?.(err, info);
        } }));
}
class SentoriErrorBoundaryInner extends Component {
    state = { error: null };
    static getDerivedStateFromError(error) {
        return { error };
    }
    componentDidCatch(error, info) {
        this.props.capture(error, info);
    }
    componentDidUpdate(prev) {
        if (this.state.error && resetKeysChanged(prev.resetKeys, this.props.resetKeys)) {
            this.setState({ error: null });
        }
    }
    reset = () => this.setState({ error: null });
    render() {
        const { error } = this.state;
        if (error) {
            const { fallback } = this.props;
            return typeof fallback === 'function'
                ? fallback({ error, reset: this.reset })
                : fallback;
        }
        return this.props.children;
    }
}
function resetKeysChanged(prev, next) {
    if (prev === next)
        return false;
    if (!prev || !next)
        return prev !== next;
    if (prev.length !== next.length)
        return true;
    for (let i = 0; i < prev.length; i++) {
        if (!Object.is(prev[i], next[i]))
            return true;
    }
    return false;
}
//# sourceMappingURL=SentoriErrorBoundary.js.map