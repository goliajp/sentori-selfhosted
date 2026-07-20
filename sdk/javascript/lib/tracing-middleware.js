// Phase 37 sub-B: server-side tracing middleware for Node web
// frameworks. Each adapter:
//
//   1. Reads the inbound W3C `traceparent` header (if any). When set,
//      this request becomes a child of the upstream trace; otherwise
//      a fresh trace is rooted.
//   2. Opens an `http.server` span on entry with method + path tags.
//   3. Finishes the span on response close, mapping HTTP status to
//      span status (5xx → error, everything else → ok).
//
// For Hono (which is async) we also wrap the handler chain with
// withSpan() so any child spans the user creates pick this one up
// as their parent. Express's classic callback middleware can't
// preserve context through next(), so it skips that step — the
// http.server span is still emitted correctly; just no automatic
// parent inheritance for child spans created inside the handler.
import { startSpan, withSpan } from '@goliapkg/sentori-core';
/** Decode W3C TraceContext. The 16-hex parent-id is zero-padded
 *  back to a 32-hex uuid so it can flow through the rest of the
 *  SDK (which uses uuid-v7 throughout). Same lossy expansion as the
 *  server-side decoder in trace_emit.rs. */
export function parseTraceparent(header) {
    if (!header)
        return null;
    const parts = header.trim().split('-');
    if (parts.length !== 4 || parts[0] !== '00')
        return null;
    const trace = parts[1] ?? '';
    const parent = parts[2] ?? '';
    if (trace.length !== 32 || parent.length !== 16)
        return null;
    if (!/^[0-9a-f]+$/i.test(trace) || !/^[0-9a-f]+$/i.test(parent))
        return null;
    const traceId = trace.slice(0, 8) + '-' + trace.slice(8, 12) + '-' +
        trace.slice(12, 16) + '-' + trace.slice(16, 20) + '-' +
        trace.slice(20, 32);
    const padded = parent + '0'.repeat(16);
    const spanId = padded.slice(0, 8) + '-' + padded.slice(8, 12) + '-' +
        padded.slice(12, 16) + '-' + padded.slice(16, 20) + '-' +
        padded.slice(20, 32);
    return { spanId: spanId.toLowerCase(), traceId: traceId.toLowerCase() };
}
/**
 * Express middleware. Mount once near the top of the chain so the
 * span wraps every downstream handler:
 *
 *     import express from 'express'
 *     import { expressTracingMiddleware } from '@goliapkg/sentori-javascript/tracing'
 *
 *     const app = express()
 *     app.use(expressTracingMiddleware())
 */
export function expressTracingMiddleware() {
    return (req, res, next) => {
        const parent = parseTraceparent(headerOf(req.headers, 'traceparent'));
        const method = (req.method ?? 'GET').toUpperCase();
        const path = req.path ?? req.url ?? '/';
        const span = startSpan('http.server', {
            name: `${method} ${path}`,
            parent: parent ?? null,
            tags: { 'http.method': method, 'http.path': path },
        });
        let finished = false;
        const onEnd = () => {
            if (finished)
                return;
            finished = true;
            const status = res.statusCode ?? 0;
            span.setTag('http.status', String(status));
            span.finish({ status: status >= 500 ? 'error' : 'ok' });
        };
        res.on('finish', onEnd);
        res.on('close', onEnd);
        next();
    };
}
/**
 * Hono middleware. Hono's middleware is async, which lets us wrap
 * the handler chain with withSpan() — child spans created in
 * downstream handlers automatically attribute to this request's span.
 *
 *     import { Hono } from 'hono'
 *     import { honoTracingMiddleware } from '@goliapkg/sentori-javascript/tracing'
 *
 *     const app = new Hono()
 *     app.use('*', honoTracingMiddleware())
 */
export function honoTracingMiddleware() {
    return async (c, next) => {
        const parent = parseTraceparent(c.req.header('traceparent'));
        const method = c.req.method.toUpperCase();
        const path = c.req.path;
        const span = startSpan('http.server', {
            name: `${method} ${path}`,
            parent: parent ?? null,
            tags: { 'http.method': method, 'http.path': path },
        });
        try {
            await withSpan(span, () => next());
            const status = c.res.status;
            span.setTag('http.status', String(status));
            span.finish({ status: status >= 500 ? 'error' : 'ok' });
        }
        catch (err) {
            if (err instanceof Error)
                span.setTag('error.message', err.message);
            span.finish({ status: 'error' });
            throw err;
        }
    };
}
/**
 * Fastify plugin-style installer. Call once with your Fastify
 * instance:
 *
 *     import Fastify from 'fastify'
 *     import { installFastifyTracing } from '@goliapkg/sentori-javascript/tracing'
 *
 *     const fastify = Fastify()
 *     installFastifyTracing(fastify)
 */
export function installFastifyTracing(fastify) {
    fastify.addHook('onRequest', (req, _reply, done) => {
        const parent = parseTraceparent(headerOf(req.headers, 'traceparent'));
        const method = (req.method ?? 'GET').toUpperCase();
        const path = req.url ?? '/';
        req.sentoriSpan = startSpan('http.server', {
            name: `${method} ${path}`,
            parent: parent ?? null,
            tags: { 'http.method': method, 'http.path': path },
        });
        done();
    });
    fastify.addHook('onResponse', (req, reply, done) => {
        const span = req.sentoriSpan;
        if (span) {
            const status = reply.statusCode;
            span.setTag('http.status', String(status));
            span.finish({ status: status >= 500 ? 'error' : 'ok' });
        }
        done();
    });
}
function headerOf(headers, name) {
    if (!headers)
        return undefined;
    const v = headers[name] ?? headers[name.toLowerCase()];
    if (Array.isArray(v))
        return v[0];
    return v;
}
//# sourceMappingURL=tracing-middleware.js.map