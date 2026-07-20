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

import { createInterface } from 'node:readline'

import type { AdminUpload } from './native-artifacts.js'

type JsonRpcRequest = {
  id?: number | string | null
  jsonrpc: '2.0'
  method: string
  params?: Record<string, unknown>
}

type JsonRpcResponse = {
  id?: number | string | null
  jsonrpc: '2.0'
  result?: unknown
  error?: { code: number; message: string; data?: unknown }
}

type ToolHandler = (args: Record<string, unknown>, ctx: McpCtx) => Promise<unknown>

type ToolDef = {
  name: string
  description: string
  inputSchema: Record<string, unknown>
  handler: ToolHandler
}

type McpCtx = AdminUpload

/** Run the MCP server over stdio. Returns when stdin closes. */
export async function runMcpServer(ctx: McpCtx): Promise<void> {
  const tools = buildTools()
  const toolMap = new Map(tools.map((t) => [t.name, t]))

  const rl = createInterface({ input: process.stdin })
  for await (const line of rl) {
    const trimmed = line.trim()
    if (!trimmed) continue
    let req: JsonRpcRequest
    try {
      req = JSON.parse(trimmed)
    } catch {
      // Per spec, malformed requests get a parse-error response with
      // null id.
      send({
        error: { code: -32700, message: 'Parse error' },
        id: null,
        jsonrpc: '2.0',
      })
      continue
    }
    // Notifications (no `id`) get no response.
    const isNotification = req.id === undefined || req.id === null
    try {
      const result = await dispatch(req, toolMap, ctx, tools)
      if (!isNotification) {
        send({ id: req.id ?? null, jsonrpc: '2.0', result })
      }
    } catch (e) {
      if (!isNotification) {
        send({
          error: { code: -32603, message: (e as Error).message },
          id: req.id ?? null,
          jsonrpc: '2.0',
        })
      }
    }
  }
}

function send(resp: JsonRpcResponse): void {
  process.stdout.write(JSON.stringify(resp) + '\n')
}

async function dispatch(
  req: JsonRpcRequest,
  toolMap: Map<string, ToolDef>,
  ctx: McpCtx,
  tools: ToolDef[],
): Promise<unknown> {
  switch (req.method) {
    case 'initialize':
      return {
        capabilities: { tools: {} },
        protocolVersion: '2024-11-05',
        serverInfo: { name: 'sentori', version: '1.0' },
      }
    case 'notifications/initialized':
      return {}
    case 'tools/list':
      return {
        tools: tools.map((t) => ({
          description: t.description,
          inputSchema: t.inputSchema,
          name: t.name,
        })),
      }
    case 'tools/call': {
      const params = (req.params ?? {}) as { arguments?: Record<string, unknown>; name?: string }
      const name = params.name
      if (typeof name !== 'string') throw new Error('missing tools/call.name')
      const tool = toolMap.get(name)
      if (!tool) throw new Error(`unknown tool: ${name}`)
      const result = await tool.handler(params.arguments ?? {}, ctx)
      return {
        content: [
          {
            text: typeof result === 'string' ? result : JSON.stringify(result, null, 2),
            type: 'text',
          },
        ],
      }
    }
    default:
      throw new Error(`method not found: ${req.method}`)
  }
}

// ── Tool implementations ─────────────────────────────────────────

async function adminGet<T>(ctx: McpCtx, path: string): Promise<T> {
  const url = `${ctx.apiUrl.replace(/\/+$/, '')}/admin/api${path}`
  const resp = await fetch(url, {
    headers: { Authorization: `Bearer ${ctx.token}` },
  })
  if (!resp.ok) throw new Error(`GET ${path} → ${resp.status} ${resp.statusText}`)
  return (await resp.json()) as T
}

async function adminPatch<T>(ctx: McpCtx, path: string, body: unknown): Promise<T> {
  const url = `${ctx.apiUrl.replace(/\/+$/, '')}/admin/api${path}`
  const resp = await fetch(url, {
    body: JSON.stringify(body),
    headers: {
      Authorization: `Bearer ${ctx.token}`,
      'Content-Type': 'application/json',
    },
    method: 'PATCH',
  })
  if (!resp.ok) throw new Error(`PATCH ${path} → ${resp.status} ${resp.statusText}`)
  return (await resp.json()) as T
}

async function adminPost<T>(ctx: McpCtx, path: string, body: unknown): Promise<T> {
  const url = `${ctx.apiUrl.replace(/\/+$/, '')}/admin/api${path}`
  const resp = await fetch(url, {
    body: body !== undefined ? JSON.stringify(body) : undefined,
    headers: {
      Authorization: `Bearer ${ctx.token}`,
      'Content-Type': 'application/json',
    },
    method: 'POST',
  })
  if (!resp.ok) throw new Error(`POST ${path} → ${resp.status} ${resp.statusText}`)
  if (resp.status === 204) return null as T
  return (await resp.json()) as T
}

async function adminPut<T>(ctx: McpCtx, path: string): Promise<T> {
  const url = `${ctx.apiUrl.replace(/\/+$/, '')}/admin/api${path}`
  const resp = await fetch(url, {
    headers: { Authorization: `Bearer ${ctx.token}` },
    method: 'PUT',
  })
  if (!resp.ok) throw new Error(`PUT ${path} → ${resp.status} ${resp.statusText}`)
  if (resp.status === 204) return null as T
  return (await resp.json()) as T
}

async function adminDelete<T>(ctx: McpCtx, path: string): Promise<T> {
  const url = `${ctx.apiUrl.replace(/\/+$/, '')}/admin/api${path}`
  const resp = await fetch(url, {
    headers: { Authorization: `Bearer ${ctx.token}` },
    method: 'DELETE',
  })
  if (!resp.ok) throw new Error(`DELETE ${path} → ${resp.status} ${resp.statusText}`)
  if (resp.status === 204) return null as T
  return (await resp.json()) as T
}

function asString(v: unknown, name: string): string {
  if (typeof v !== 'string' || v.length === 0) {
    throw new Error(`${name} is required (string)`)
  }
  return v
}

function asOptionalString(v: unknown): string | undefined {
  if (v === undefined || v === null) return undefined
  if (typeof v !== 'string') throw new Error('expected string')
  return v
}

export function buildTools(): ToolDef[] {
  return [
    {
      description:
        'List issues for a Sentori project, with optional status / priority / label filters.',
      handler: async (args, ctx) => {
        const projectId = asString(args.projectId, 'projectId')
        const usp = new URLSearchParams()
        const status = asOptionalString(args.status) ?? 'any'
        usp.set('status', status)
        if (args.priority) usp.set('priority', String(args.priority))
        if (args.label) usp.set('labels', String(args.label))
        if (typeof args.limit === 'number') usp.set('limit', String(args.limit))
        return await adminGet(ctx, `/projects/${projectId}/issues?${usp}`)
      },
      inputSchema: {
        properties: {
          label: { type: 'string' },
          limit: { type: 'number' },
          priority: { type: 'string' },
          projectId: { type: 'string' },
          status: { type: 'string' },
        },
        required: ['projectId'],
        type: 'object',
      },
      name: 'sentori_issue_list',
    },
    {
      description: 'Get full detail for one Sentori issue including its activity feed.',
      handler: async (args, ctx) => {
        const projectId = asString(args.projectId, 'projectId')
        const issueId = asString(args.issueId, 'issueId')
        const [issue, activity] = await Promise.all([
          adminGet(ctx, `/projects/${projectId}/issues/${issueId}`),
          adminGet(ctx, `/projects/${projectId}/issues/${issueId}/activity`),
        ])
        return { activity, issue }
      },
      inputSchema: {
        properties: {
          issueId: { type: 'string' },
          projectId: { type: 'string' },
        },
        required: ['projectId', 'issueId'],
        type: 'object',
      },
      name: 'sentori_issue_get',
    },
    {
      description: 'Add a comment to a Sentori issue.',
      handler: async (args, ctx) => {
        const projectId = asString(args.projectId, 'projectId')
        const issueId = asString(args.issueId, 'issueId')
        const body = asString(args.body, 'body')
        return await adminPost(ctx, `/projects/${projectId}/issues/${issueId}/comments`, {
          body,
        })
      },
      inputSchema: {
        properties: {
          body: { type: 'string' },
          issueId: { type: 'string' },
          projectId: { type: 'string' },
        },
        required: ['projectId', 'issueId', 'body'],
        type: 'object',
      },
      name: 'sentori_issue_comment',
    },
    {
      description:
        'Transition an issue to a new status (active|silenced|muted|resolved|closed).',
      handler: async (args, ctx) => {
        const projectId = asString(args.projectId, 'projectId')
        const issueId = asString(args.issueId, 'issueId')
        const status = asString(args.status, 'status')
        return await adminPatch(ctx, `/projects/${projectId}/issues/${issueId}`, {
          status,
        })
      },
      inputSchema: {
        properties: {
          issueId: { type: 'string' },
          projectId: { type: 'string' },
          status: {
            enum: ['active', 'silenced', 'muted', 'resolved', 'closed'],
            type: 'string',
          },
        },
        required: ['projectId', 'issueId', 'status'],
        type: 'object',
      },
      name: 'sentori_issue_transition',
    },
    {
      description: 'Assign an issue to a user, or pass userId=null to unassign.',
      handler: async (args, ctx) => {
        const projectId = asString(args.projectId, 'projectId')
        const issueId = asString(args.issueId, 'issueId')
        const userId = args.userId === null ? null : asOptionalString(args.userId)
        return await adminPatch(ctx, `/projects/${projectId}/issues/${issueId}`, {
          assigneeUserId: userId ?? null,
        })
      },
      inputSchema: {
        properties: {
          issueId: { type: 'string' },
          projectId: { type: 'string' },
          userId: { type: ['string', 'null'] },
        },
        required: ['projectId', 'issueId', 'userId'],
        type: 'object',
      },
      name: 'sentori_issue_assign',
    },
    {
      description: 'Set the triage priority on an issue.',
      handler: async (args, ctx) => {
        const projectId = asString(args.projectId, 'projectId')
        const issueId = asString(args.issueId, 'issueId')
        const priority = asString(args.priority, 'priority')
        return await adminPatch(ctx, `/projects/${projectId}/issues/${issueId}`, {
          priority,
        })
      },
      inputSchema: {
        properties: {
          issueId: { type: 'string' },
          priority: { enum: ['p0', 'p1', 'p2', 'p3'], type: 'string' },
          projectId: { type: 'string' },
        },
        required: ['projectId', 'issueId', 'priority'],
        type: 'object',
      },
      name: 'sentori_issue_set_priority',
    },
    {
      description: 'Replace the label set on an issue. Pass [] to clear all.',
      handler: async (args, ctx) => {
        const projectId = asString(args.projectId, 'projectId')
        const issueId = asString(args.issueId, 'issueId')
        if (!Array.isArray(args.labels)) throw new Error('labels must be string[]')
        const labels = args.labels.map((l) => {
          if (typeof l !== 'string') throw new Error('each label must be a string')
          return l
        })
        return await adminPatch(ctx, `/projects/${projectId}/issues/${issueId}`, {
          labels,
        })
      },
      inputSchema: {
        properties: {
          issueId: { type: 'string' },
          labels: { items: { type: 'string' }, type: 'array' },
          projectId: { type: 'string' },
        },
        required: ['projectId', 'issueId', 'labels'],
        type: 'object',
      },
      name: 'sentori_issue_set_labels',
    },
    {
      description:
        'Subscribe (watch=true) or unsubscribe (watch=false) the configured caller to an issue.',
      handler: async (args, ctx) => {
        const projectId = asString(args.projectId, 'projectId')
        const issueId = asString(args.issueId, 'issueId')
        const watch = args.watch === true
        if (watch) {
          return await adminPut(ctx, `/projects/${projectId}/issues/${issueId}/watch`)
        }
        return await adminDelete(ctx, `/projects/${projectId}/issues/${issueId}/watch`)
      },
      inputSchema: {
        properties: {
          issueId: { type: 'string' },
          projectId: { type: 'string' },
          watch: { type: 'boolean' },
        },
        required: ['projectId', 'issueId', 'watch'],
        type: 'object',
      },
      name: 'sentori_issue_watch',
    },
  ]
}
