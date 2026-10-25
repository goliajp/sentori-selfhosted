import { parseArgs } from 'node:util';
import { readFileSync } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { showProject } from './apps.js';
export const WEB_HELP = `Web App monitoring (API token; all output is JSON):
  sentori web performance --app <uuid> [--hours 24] [--route /template] [--device mobile|desktop]
                         [--release <r>] [--environment <env>]
  sentori web checks --app <uuid>
  sentori web create --config <check.json>
  sentori web update --id <uuid> --version <n> --config <check.json>
  sentori web delete --id <uuid> --version <n>
  sentori web run --id <check-uuid> [--trigger manual|deployment] [--app <uuid> --wait --strict]
  sentori web runs --app <uuid> [--id <run-uuid>] [--strict]
  Mutations accept --command-id <uuid> for safe retries. --project <uuid> verifies the token's project.
  Without --strict, findings do not change exit 0. Strict: 1=site failed, 2=monitor/API/timeout error.
  Synthetic measurements are laboratory results; they are never real-user percentiles or INP.
`;
export async function runWeb(argv) {
    try {
        const [command, ...args] = argv;
        if (command === '--help' || command === '-h') {
            console.log(WEB_HELP);
            return 0;
        }
        const { values: v } = parseArgs({ args, options: {
                help: { type: 'boolean', short: 'h' }, json: { type: 'boolean' }, 'api-url': { type: 'string' }, token: { type: 'string' }, app: { type: 'string' },
                hours: { type: 'string' }, route: { type: 'string' }, device: { type: 'string' }, release: { type: 'string' }, environment: { type: 'string' },
                id: { type: 'string' }, version: { type: 'string' }, config: { type: 'string' }, trigger: { type: 'string' }, wait: { type: 'boolean' }, strict: { type: 'boolean' },
                'command-id': { type: 'string' }, project: { type: 'string' },
            } });
        if (!command || v.help) {
            console.log(WEB_HELP);
            return 0;
        }
        const token = v.token ?? process.env.SENTORI_API_TOKEN ?? process.env.SENTORI_TOKEN;
        const apiUrl = v['api-url'] ?? process.env.SENTORI_API_URL ?? 'https://sentori.golia.jp';
        if (!token)
            throw new Error('Set SENTORI_API_TOKEN to the project API token');
        if (v.project) {
            const { project } = await showProject({ apiUrl, token });
            if (project.id !== v.project)
                throw new Error(`Project mismatch: expected ${v.project}, received ${project.id}`);
        }
        async function request(path, body) {
            const response = await fetch(apiUrl.replace(/\/+$/, '') + path, { method: body ? 'POST' : 'GET',
                headers: { Authorization: `Bearer ${token}`, 'Content-Type': 'application/json' }, body: body ? JSON.stringify(body) : undefined,
                redirect: 'error', signal: AbortSignal.timeout(30_000) });
            if (!response.ok)
                throw new Error(`Web monitoring API returned HTTP ${response.status}`);
            return await response.json();
        }
        const required = (key) => { if (!v[key])
            throw new Error(`--${key} is required`); return v[key]; };
        let result;
        if (command === 'performance') {
            const q = new URLSearchParams({ app: required('app') });
            for (const key of ['hours', 'route', 'device', 'release', 'environment'])
                if (v[key])
                    q.set(key, v[key]);
            result = await request('/api/web/performance?' + q);
        }
        else if (command === 'checks' || command === 'runs') {
            const q = new URLSearchParams({ app: required('app') });
            if (v.id)
                q.set('id', v.id);
            if (v.strict && (command !== 'runs' || !v.id))
                throw new Error('--strict requires runs --id or run --wait');
            result = await request(`/api/web/${command}?${q}`);
        }
        else if (['create', 'update', 'delete', 'run'].includes(command)) {
            if (v.strict && (command !== 'run' || !v.wait))
                throw new Error('--strict requires run --wait');
            if (v.wait)
                required('app');
            const body = { action: command, commandId: v['command-id'] ?? randomUUID() };
            if (command !== 'create')
                body.id = required('id');
            if (command === 'create' || command === 'update')
                body.configuration = JSON.parse(readFileSync(required('config'), 'utf8'));
            if (command === 'update' || command === 'delete') {
                body.expectedVersion = Number(required('version'));
                if (!Number.isSafeInteger(body.expectedVersion) || body.expectedVersion < 1)
                    throw new Error('--version must be a positive integer');
            }
            if (command === 'run')
                body.trigger = v.trigger ?? 'manual';
            // Preserve this ID before sending: a transport failure may happen after the server commits.
            console.error(`commandId=${body.commandId}`);
            result = await request('/api/web/checks', body);
            if (command === 'run' && v.wait) {
                const end = Date.now() + 330_000;
                while (Date.now() < end) {
                    const runs = await request('/api/web/runs?' + new URLSearchParams({ app: v.app, id: result.id }));
                    const run = runs.runs?.[0];
                    if (run && !['queued', 'running'].includes(run.status)) {
                        result = runs;
                        break;
                    }
                    await new Promise(resolve => setTimeout(resolve, 2000));
                }
                if (!result.runs)
                    throw new Error(`Run ${result.id} did not finish before the wait timeout; inspect it with web runs`);
            }
        }
        else
            throw new Error('Unknown web command; use sentori web --help');
        console.log(JSON.stringify(result, null, 2));
        if (v.strict) {
            const run = result.runs?.[0];
            if (!run || ['error', 'cancelled', 'queued', 'running'].includes(run.status))
                return 2;
            return run.status === 'failed' ? 1 : 0;
        }
        return 0;
    }
    catch (error) {
        console.error(error.message);
        return 2;
    }
}
//# sourceMappingURL=web.js.map