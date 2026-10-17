async function read(config, path) {
    const response = await fetch(`${config.apiUrl.replace(/\/+$/, '')}${path}`, {
        headers: { Authorization: `Bearer ${config.token}` },
        redirect: 'error',
        signal: AbortSignal.timeout(30_000),
    });
    if (!response.ok)
        throw new Error(`Sentori API returned ${response.status}`);
    return await response.json();
}
export function listApps(config) {
    return read(config, '/api/apps');
}
export function showProject(config) {
    return read(config, '/api/project');
}
//# sourceMappingURL=apps.js.map