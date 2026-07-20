import type { AdminUpload } from './native-artifacts.js';
type ToolHandler = (args: Record<string, unknown>, ctx: McpCtx) => Promise<unknown>;
type ToolDef = {
    name: string;
    description: string;
    inputSchema: Record<string, unknown>;
    handler: ToolHandler;
};
type McpCtx = AdminUpload;
/** Run the MCP server over stdio. Returns when stdin closes. */
export declare function runMcpServer(ctx: McpCtx): Promise<void>;
export declare function buildTools(): ToolDef[];
export {};
//# sourceMappingURL=mcp.d.ts.map