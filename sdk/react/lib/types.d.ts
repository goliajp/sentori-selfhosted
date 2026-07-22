import type { Breadcrumb, BreadcrumbType, CaptureExtras, CommonInitOptions, Tags, User } from '@goliapkg/sentori-core';
export type SentoriReactConfig = CommonInitOptions & {
    /**
     * Disable the JS SDK's automatic window/process error hooks. Default
     * is to leave them on so non-React errors (network handlers, top-
     * level promises) still capture. Set to false if you want
     * SentoriErrorBoundary to be the only entry point.
     */
    enableGlobalHooks?: boolean;
};
export type SentoriContextValue = {
    /** Append a breadcrumb to the per-process ring buffer. */
    addBreadcrumb: (type: BreadcrumbType, data?: Record<string, unknown>) => void;
    /**
     * Capture any thrown value. Plain `Error` is the happy path; non-
     * Error values get wrapped so the dashboard still sees a stack.
     */
    captureError: (error: Error, extras?: CaptureExtras) => void;
    /** Same as captureError. Kept for parity with the JS SDK. */
    captureException: (error: Error, extras?: CaptureExtras) => void;
    /** Whether SentoriProvider has finished its one-shot init. */
    initialised: boolean;
    /** Attach a stable user identifier to subsequent events. */
    setUser: (user: null | User) => void;
    /** Replace the entire tag set on subsequent events. */
    setTags: (tags: Tags | null) => void;
};
export type { Breadcrumb, BreadcrumbType, CaptureExtras, Tags, User };
//# sourceMappingURL=types.d.ts.map