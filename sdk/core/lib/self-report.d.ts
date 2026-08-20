/**
 * Self-report — observability for SDK-internal failures.
 *
 * When `safeFn` catches an error from inside a public API, it calls
 * `reportInternal(api, err)` so the failure becomes observable in
 * the dashboard without ever surfacing to the host app.
 *
 * Gating: a leaky-bucket circuit breaker caps self-reports at
 * `FAILURE_BUDGET_PER_MIN` per rolling minute. Beyond that the
 * function silently does nothing. This protects the host app from
 * recursion storms: if the transport itself is broken, our
 * self-report attempt could trigger another internal failure, which
 * would trigger another self-report — without the cap we'd spin.
 *
 * The whole body is also wrapped in its own try/catch — recursive
 * failure inside `reportInternal` is silent. The NEVER rule wins
 * over our own observability.
 *
 * `setInternalReporter` is the SDK's hook: the framework SDK (e.g.
 * RN's transport) calls it once during `init()` to wire how the
 * `kind: nearCrash` event actually gets enqueued. Core itself
 * stays transport-agnostic.
 */
type InternalReporter = (payload: {
    api: string;
    message: string;
    errorName?: string;
    stack?: string;
}) => void;
/** Framework SDK calls this once during `init()` to register the
 *  transport-level enqueue function. Core stays transport-agnostic. */
export declare function setInternalReporter(reporter: InternalReporter | null): void;
/** Returns true if the circuit is open (we're suppressing reports). */
export declare function isCircuitOpen(): boolean;
/** Test hook — reset the circuit breaker so unit tests start fresh. */
export declare function __resetCircuitForTests(): void;
/**
 * Record an internal SDK failure. The host app never sees this
 * call's behaviour — even a recursive throw is swallowed.
 */
export declare function reportInternal(api: string, err: unknown): void;
export {};
//# sourceMappingURL=self-report.d.ts.map