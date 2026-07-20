// Drives the @sentori/react-native SDK transport from bun to verify
// the SDK↔server protocol without booting a simulator.

import { sentori } from '@sentori/react-native';

const token = process.env.SENTORI_TOKEN;
const ingestUrl = process.env.INGEST_URL;

if (!token || !ingestUrl) {
  console.error('SENTORI_TOKEN and INGEST_URL must be set');
  process.exit(1);
}

sentori.init({
  token,
  release: 'sentori-e2e@1.0.0+1',
  environment: 'test',
  ingestUrl,
});

sentori.captureError(new Error('e2e smoke test'));

// Wait for the batcher's flush window (5s) plus a small buffer.
await new Promise((r) => setTimeout(r, 6000));

console.log('OK: event sent');
