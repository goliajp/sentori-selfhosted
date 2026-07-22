/**
 * Phase 26 sub-B: SDK-side session tracking glue.
 *
 * Holds a singleton `SessionTracker` keyed off the package's lifetime.
 * `initSentori` calls `start()`; capture promotes status; pagehide /
 * graceful Node exit calls `end()`.
 */
export declare function startSession(): void;
export declare function endSession(status?: 'exited'): void;
export declare function markSessionErrored(): void;
export declare function markSessionCrashed(): void;
/** Test helper — drops the singleton so each test starts clean. */
export declare function _resetSessionTrackerForTesting(): void;
//# sourceMappingURL=session-tracker.d.ts.map