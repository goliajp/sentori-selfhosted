/**
 * Normalize a request URL for use as a span `name`. Keeps
 * scheme + host + path (with id-like segments → `{id}`); drops the
 * query string and fragment. Falls back to treating the input as a
 * bare path if it isn't a parseable absolute URL (relative requests).
 */
export declare function normalizeUrl(url: string): string;
//# sourceMappingURL=url.d.ts.map