// v1.2 W9 — Sentori MCP server.
//
// Stdio JSON-RPC 2.0 transport per the Model Context Protocol spec
// (https://modelcontextprotocol.io/). LLM clients (Claude Code,
// custom agents) spawn `sentori-cli mcp serve` as a subprocess and
// pipe MCP messages over stdin/stdout. Each tool call translates
// 1:1 to the existing admin API; the MCP layer is pure protocol
// glue + auth-passthrough.
//
// Why CLI-hosted instead of server-hosted MCP:
//   - The Sentori server exposes admin endpoints over HTTPS already;
//     spinning up a parallel MCP endpoint would duplicate auth +
//     route boilerplate.
//   - LLM clients expect MCP servers to be local stdio subprocesses
//     (Claude Code config, gptme, etc.). The CLI is the natural
//     binary to embed it in — the operator already has it installed
//     and a token configured.
//   - Easier to ship + version with the rest of the CLI.
import { createInterface } from 'node:readline';
/** Run the MCP server over stdio. Returns when stdin closes. */
export async function runMcpServer(ctx) {
    const tools = buildTools();
    const toolMap = new Map(tools.map((t) => [t.name, t]));
    const rl = createInterface({ input: process.stdin });
    for await (const line of rl) {
        const trimmed = line.trim();
        if (!trimmed)
            continue;
        let req;
        try {
            req = JSON.parse(trimmed);
        }
        catch {
            // Per spec, malformed requests get a parse-error response with
            // null id.
            send({
                error: { code: -32700, message: 'Parse error' },
                id: null,
                jsonrpc: '2.0',
            });
            continue;
        }
        // Notifications (no `id`) get no response.
        const isNotification = req.id === undefined || req.id === null;
        try {
            const result = await dispatch(req, toolMap, ctx, tools);
            if (!isNotification) {
                send({ id: req.id ?? null, jsonrpc: '2.0', result });
            }
        }
        catch (e) {
            if (!isNotification) {
                send({
                    error: { code: -32603, message: e.message },
                    id: req.id ?? null,
                    jsonrpc: '2.0',
                });
            }
        }
    }
}
function send(resp) {
    process.stdout.write(JSON.stringify(resp) + '\n');
}
async function dispatch(req, toolMap, ctx, tools) {
    switch (req.method) {
        case 'initialize':
            return {
                capabilities: { tools: {} },
                protocolVersion: '2024-11-05',
                serverInfo: { name: 'sentori', version: '1.0' },
            };
        case 'notifications/initialized':
            return {};
        case 'tools/list':
            return {
                tools: tools.map((t) => ({
                    description: t.description,
                    inputSchema: t.inputSchema,
                    name: t.name,
                })),
            };
        case 'tools/call': {
            const params = (req.params ?? {});
            const name = params.name;
            if (typeof name !== 'string')
                throw new Error('missing tools/call.name');
            const tool = toolMap.get(name);
            if (!tool)
                throw new Error(`unknown tool: ${name}`);
            const result = await tool.handler(params.arguments ?? {}, ctx);
            return {
                content: [
                    {
                        text: typeof result === 'string' ? result : JSON.stringify(result, null, 2),
                        type: 'text',
                    },
                ],
            };
        }
        default:
            throw new Error(`method not found: ${req.method}`);
    }
}
// ── Tool implementations — the /api closed loop ──────────────────
//
// Four tools, mirroring exactly what an agent needs (design.md §9):
// pick work, pull the evidence, write back, resolve. The bundle is
// the product; everything else is triage plumbing.
async function apiGet(ctx, path, raw = false) {
    const url = `${ctx.apiUrl.replace(/\/+$/, '')}${path}`;
    const resp = await fetch(url, { headers: { Authorization: `Bearer ${ctx.token}` } });
    if (!resp.ok)
        throw new Error(`GET ${path} → ${resp.status} ${resp.statusText}`);
    return raw ? resp.text() : resp.json();
}
async function apiPost(ctx, path, body) {
    const url = `${ctx.apiUrl.replace(/\/+$/, '')}${path}`;
    const resp = await fetch(url, {
        body: JSON.stringify(body),
        headers: {
            Authorization: `Bearer ${ctx.token}`,
            'Content-Type': 'application/json',
        },
        method: 'POST',
    });
    if (!resp.ok)
        throw new Error(`POST ${path} → ${resp.status} ${resp.statusText}`);
    return resp.json();
}
function asString(v, name) {
    if (typeof v !== 'string' || v.length === 0)
        throw new Error(`${name} must be a non-empty string`);
    return v;
}
export function buildTools() {
    return [
        {
            name: 'sentori_issue_list',
            description: 'List issues (default: open, ordered by objective importance — regressed first, then breadth × depth). Filter with status (open|resolved|ignored) and kind (error|warn|trace|assert|probe).',
            inputSchema: {
                type: 'object',
                properties: {
                    status: { type: 'string', enum: ['open', 'resolved', 'ignored'] },
                    kind: { type: 'string', enum: ['assert', 'error', 'probe', 'trace', 'warn'] },
                },
            },
            handler: async (args, ctx) => {
                const q = new URLSearchParams();
                if (typeof args.status === 'string')
                    q.set('status', args.status);
                if (typeof args.kind === 'string')
                    q.set('kind', args.kind);
                const qs = q.toString();
                return apiGet(ctx, `/api/issues${qs ? `?${qs}` : ''}`);
            },
        },
        {
            name: 'sentori_issue_bundle',
            description: 'Fetch the full evidence bundle for one issue as markdown — read it and you have everything needed to fix: stack, user timeline, environment, distribution, guard-probe status.',
            inputSchema: {
                type: 'object',
                properties: { issueId: { type: 'string' } },
                required: ['issueId'],
            },
            handler: async (args, ctx) => apiGet(ctx, `/api/issues/${encodeURIComponent(asString(args.issueId, 'issueId'))}/bundle`, true),
        },
        {
            name: 'sentori_issue_note',
            description: 'Append a note to an issue — write back what you did ("fixed in abc123, probe SENT-42 planted").',
            inputSchema: {
                type: 'object',
                properties: { issueId: { type: 'string' }, body: { type: 'string' } },
                required: ['issueId', 'body'],
            },
            handler: async (args, ctx) => apiPost(ctx, `/api/issues/${encodeURIComponent(asString(args.issueId, 'issueId'))}/notes`, {
                body: asString(args.body, 'body'),
            }),
        },
        {
            name: 'sentori_issue_resolve',
            description: 'Resolve an issue, anchored on the release that carries the fix — only a recurrence in that release or newer counts as a regression.',
            inputSchema: {
                type: 'object',
                properties: { issueId: { type: 'string' }, release: { type: 'string' } },
                required: ['issueId'],
            },
            handler: async (args, ctx) => apiPost(ctx, `/api/issues/${encodeURIComponent(asString(args.issueId, 'issueId'))}/resolve`, {
                release: typeof args.release === 'string' ? args.release : undefined,
            }),
        },
    ];
}
//# sourceMappingURL=mcp.js.map