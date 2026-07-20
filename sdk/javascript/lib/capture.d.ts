import { type CaptureMessageOptions, type LinkBy, type TrailStep } from '@goliapkg/sentori-core';
import type { CaptureExtras, User } from './types.js';
/**
 * v2.0 — set a single scope tag that's merged onto every subsequent
 * capture. Per-call `extras.tags` / `opts.tags` win over scope tags.
 *
 *     sentori.setTag('rollout', 'dark-mode-v2')
 *     sentori.captureException(err)  // event.tags carries rollout
 */
export declare function setTag(key: string, value: string): void;
/**
 * v2.0 — bulk variant of setTag. Existing tags are merged with
 * the input record; pass `{}` to clear (`Object.assign` style).
 */
export declare function setTags(record: Record<string, string>): void;
export declare const __resetScopeForTests: () => void;
/**
 * Phase 46 — record a step into the session-trail buffer. The buffer
 * is a fixed-size FIFO; pushing past capacity drops the oldest.
 * Uploaded as a `sessionTrail` attachment on the next
 * `captureException` only when `init({ capture: { sessionTrail:
 * true } })` is on.
 */
export declare function captureStep(label: string, opts?: Partial<TrailStep>): void;
export declare function __resetTrailForTests(): void;
/**
 * Attach a stable user identifier to events captured after this call.
 *
 * PII policy: User shape is `{ id?, anonymous? }` only — no email,
 * name, IP, or other identifying fields. The server schema enforces
 * the same shape; extras would be rejected with `validationFailed`.
 */
/**
 * v2.3 — accepts optional `linkBy` map for cross-project lookup
 * (sentori-core/identity.ts). Each value is hashed client-side via
 * SubtleCrypto and committed to scope when the async hash settles.
 * Raw values never leave the device. See the v2.3 redesign doc §5.
 */
type SetUserInput = (User & {
    linkBy?: LinkBy;
}) | null;
export declare function setUser(input: SetUserInput): void;
export declare function getUser(): User | null;
export declare function captureError(error: Error, extras?: CaptureExtras): void;
export declare const captureException: typeof captureError;
/**
 * Manually report an issue without an Error instance.
 *
 * Routes to the dashboard Issues module — distinct from `track`
 * (analytics) and `recordMetric` (numeric). Use for "operator
 * should look at this" signals: a fallback that fired, an unexpected
 * state, a feature flag rollout that crossed a threshold.
 *
 *     sentori.captureMessage('Payment provider returned 500, used fallback')
 *     sentori.captureMessage('Detected impossible state in session reducer', {
 *       level: 'error',
 *       tags: { reducer: 'session' },
 *     })
 *
 * Wrapped in `safeFn` per the NEVER rule — any internal failure is
 * swallowed and (optionally) self-reported; the host app never sees
 * a thrown error.
 */
export declare const captureMessage: (message: string, opts?: CaptureMessageOptions | undefined) => void | undefined;
export {};
//# sourceMappingURL=capture.d.ts.map