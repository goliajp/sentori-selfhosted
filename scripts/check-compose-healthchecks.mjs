// Every healthcheck in the shipped compose file must be exec form.
//
// CMD-SHELL runs the test through /bin/sh. An image built on scratch
// carries its server and nothing else — `pg_isready` is present, a
// shell is not — so the test cannot run, the container never reports
// healthy, and `depends_on: condition: service_healthy` holds the
// whole stack down before a single statement is issued. The failure
// reads as "database is unhealthy", which points at the database
// rather than at the quoting of the check.
//
// Cheap to satisfy: exec form calls the binary directly and works on
// both a distribution image and a scratch one.
import { readFileSync } from 'node:fs';

const FILE = 'self-hosted/docker/docker-compose.yml';
const text = readFileSync(new URL(`../${FILE}`, import.meta.url), 'utf8');

const tests = [...text.matchAll(/^\s*test:\s*(.+)$/gm)].map((m) => m[1].trim());

if (tests.length === 0) {
  console.error(
    `✗ found no healthcheck tests in ${FILE}. This checker is broken, ` +
      `not the tree — it has nothing to judge and must not pass.`,
  );
  process.exit(1);
}

const shellForm = tests.filter((t) => t.includes('CMD-SHELL'));
if (shellForm.length > 0) {
  console.error(`✗ ${FILE}: healthcheck uses CMD-SHELL:`);
  for (const t of shellForm) console.error(`    ${t}`);
  console.error(
    '  An image without /bin/sh cannot answer it, and the stack then ' +
      'never starts. Use exec form: ["CMD", "pg_isready", "-U", "sentori"].',
  );
  process.exit(1);
}

console.log(`✓ ${tests.length} healthcheck(s), all exec form`);
