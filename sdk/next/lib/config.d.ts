import type { CommonInitOptions } from '@goliapkg/sentori-core';
export type Side = 'client' | 'server';
export type SentoriNextConfig = Partial<CommonInitOptions> & {
    /** Override the env-resolution. Useful in tests. */
    envOverride?: Record<string, string | undefined>;
};
/**
 * Resolve a complete CommonInitOptions from env + explicit overrides.
 * `side` controls the env prefix; explicit values from `cfg` always
 * win.
 *
 * Throws when a required field is unresolved on either side — the
 * caller can catch + log at boot time and continue without Sentori
 * if the env isn't wired yet.
 */
export declare function resolveConfig(side: Side, cfg?: SentoriNextConfig): CommonInitOptions;
//# sourceMappingURL=config.d.ts.map