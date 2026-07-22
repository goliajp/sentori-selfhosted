import { startSpan, type SpanContextLike } from '@goliapkg/sentori-core';
/** Decode W3C TraceContext. The 16-hex parent-id is zero-padded
 *  back to a 32-hex uuid so it can flow through the rest of the
 *  SDK (which uses uuid-v7 throughout). Same lossy expansion as the
 *  server-side decoder in trace_emit.rs. */
export declare function parseTraceparent(header?: null | string): null | SpanContextLike;
type ExpressLikeReq = {
    headers?: Record<string, string | string[] | undefined>;
    method?: string;
    path?: string;
    url?: string;
};
type ExpressLikeRes = {
    on(event: 'finish' | 'close', listener: () => void): void;
    statusCode?: number;
};
type ExpressNext = (err?: unknown) => void;
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
export declare function expressTracingMiddleware(): (req: ExpressLikeReq, res: ExpressLikeRes, next: ExpressNext) => void;
type HonoLikeContext = {
    req: {
        header(name: string): string | undefined;
        method: string;
        path: string;
    };
    res: {
        status: number;
    };
};
type HonoNext = () => Promise<void>;
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
export declare function honoTracingMiddleware(): (c: HonoLikeContext, next: HonoNext) => Promise<void>;
type FastifyRequest = {
    headers: Record<string, string | string[] | undefined>;
    method?: string;
    url?: string;
    /** Slot we use to ferry the span across the two hooks. */
    sentoriSpan?: ReturnType<typeof startSpan>;
};
type FastifyReply = {
    statusCode: number;
};
type FastifyInstance = {
    addHook(name: 'onRequest', handler: (req: FastifyRequest, _reply: FastifyReply, done: () => void) => void): unknown;
    addHook(name: 'onResponse', handler: (req: FastifyRequest, reply: FastifyReply, done: () => void) => void): unknown;
};
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
export declare function installFastifyTracing(fastify: FastifyInstance): void;
export {};
//# sourceMappingURL=tracing-middleware.d.ts.map