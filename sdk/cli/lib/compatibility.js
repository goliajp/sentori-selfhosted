import { readFileSync } from 'node:fs';
/** Read the installed package, so a release bump cannot leave a stale constant. */
export const CLI_VERSION = JSON.parse(readFileSync(new URL('../package.json', import.meta.url), 'utf8')).version;
/** Capabilities, not a server version comparison: backports may share versions. */
export function checkCompatibility(result, sourceZip) {
    const info = result.compatibility;
    if (info === undefined) {
        if (sourceZip)
            throw new Error('This server does not advertise source ZIP support. No files were uploaded. Upgrade the Sentori server to a build advertising artifact protocol 1 and sourceBundleFormats: ["zip"], then run sentori doctor. Existing CLI 1.5 pipelines may continue using legacy JSON sources.');
        return 'Legacy artifact API: server capabilities are not advertised. Source maps, dSYM and mapping uploads remain available; verification still requires matching SHA-256 bytes and usable: true.';
    }
    if (!info || typeof info.serverVersion !== 'string' || info.artifactProtocol !== 1
        || !Array.isArray(info.sourceBundleFormats) || !Array.isArray(info.verification)
        || !info.verification.includes('sha256') || !info.verification.includes('format')) {
        throw new Error('Unsupported server artifact protocol or verification capabilities. No files were uploaded. Install a compatible CLI/server pair; use sentori --version and sentori doctor to inspect it.');
    }
    if (sourceZip && !info.sourceBundleFormats.includes('zip'))
        throw new Error(`Sentori server ${info.serverVersion} does not support source ZIPs. No files were uploaded. Upgrade the server before uploading sources.`);
    return `Sentori CLI ${CLI_VERSION}; server ${info.serverVersion}; artifact protocol 1; source ZIP ${info.sourceBundleFormats.includes('zip') ? 'supported' : 'unavailable'}.`;
}
//# sourceMappingURL=compatibility.js.map