// What the console tells an operator they just pasted.
//
// `recognise` decides which validator runs, which field an error
// points at, and whether the twin file gets named. Getting
// `google-services.json` wrong here does not produce a wrong colour —
// it produces "invalid JSON", which sends someone back to the same
// download button they just used, and they press it again.
//
// So: the twin cases are the tests. The happy paths are almost free.

import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const { recognise, PROVIDER_SPECS, verdictTone } = await import(
  join(root, 'src/lib/push-credentials.ts')
);

const problems = [];
let checked = 0;

/** Assert one field of one recognition. */
function is(name, actual, expected) {
  checked += 1;
  if (actual !== expected) problems.push(`${name}: got ${JSON.stringify(actual)}, want ${JSON.stringify(expected)}`);
}

// A Google service account, shaped as Google ships it.
const SERVICE_ACCOUNT = JSON.stringify({
  type: 'service_account',
  project_id: 'insight-mobile-42',
  private_key_id: 'abc',
  private_key: '-----BEGIN PRIVATE KEY-----\nMII...\n-----END PRIVATE KEY-----\n',
  client_email: 'firebase-adminsdk@insight-mobile-42.iam.gserviceaccount.com',
});

// The twin: the app-side config, downloaded from the same console,
// one screen away.
const GOOGLE_SERVICES = JSON.stringify({
  project_info: {
    project_number: '1234567890',
    project_id: 'insight-mobile-42',
    storage_bucket: 'insight-mobile-42.appspot.com',
  },
  client: [{ client_info: { mobilesdk_app_id: '1:123:android:abc' } }],
  configuration_version: '1',
});

const PEM_KEY =
  '-----BEGIN PRIVATE KEY-----\nMIGTAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBHkwdwIBAQQg\n-----END PRIVATE KEY-----\n';
const PEM_CERT =
  '-----BEGIN CERTIFICATE-----\nMIIFazCCA1OgAwIBAgIRAIIQz7DSQONZRGPgu2OCiwAw\n-----END CERTIFICATE-----\n';
const PEM_PUBLIC =
  '-----BEGIN PUBLIC KEY-----\nMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE\n-----END PUBLIC KEY-----\n';

// --- the twins, which are the whole point -------------------------
{
  const r = recognise(GOOGLE_SERVICES, 'google-services.json');
  is('google-services.json is named', r.issue, 'google-services-json');
  is('google-services.json is not taken for a credential', r.provider, undefined);
}
{
  const r = recognise(SERVICE_ACCOUNT, 'insight-mobile-42-a1b2c3.json');
  is('service account → fcm', r.provider, 'fcm');
  is('service account → project', r.projectId, 'insight-mobile-42');
  is('service account has no issue', r.issue, undefined);
}

// --- Apple ---------------------------------------------------------
{
  const r = recognise(PEM_KEY, 'AuthKey_9F8B7C6D5E.p8');
  is('a private key → apns', r.provider, 'apns');
  // The Key ID is otherwise typed by hand off a web page.
  is('key id comes out of the file name', r.keyId, '9F8B7C6D5E');
}
{
  // Apple's other .p8 is byte-identical in shape. It must NOT be
  // flagged here — claiming to tell them apart would be a lie, and
  // the probe is what settles it.
  const r = recognise(PEM_KEY, 'AuthKey_1A2B3C4D5E.p8');
  is('the App Store Connect twin is not falsely accused', r.issue, undefined);
}
{
  const r = recognise(PEM_KEY, 'authkey_9f8b7c6d5e.p8');
  is('a lowercased file name still yields the key id', r.keyId, '9F8B7C6D5E');
}
{
  const r = recognise(PEM_KEY, 'AuthKey_9F8B7C6D5E (1).p8');
  // A second download in the same folder. No key id rather than a
  // wrong one.
  is('a browser-renamed copy yields no key id', r.keyId, undefined);
  is('...but is still recognised as a key', r.provider, 'apns');
}

// --- the other wrong files ----------------------------------------
is('a certificate is named', recognise(PEM_CERT, 'aps.pem').issue, 'is-certificate');
is('a public key is named', recognise(PEM_PUBLIC).issue, 'is-public-key');
is('unparseable json is named', recognise('{oh no').issue, 'json-not-service-account');
is('random text is unrecognised', recognise('hunter2').issue, 'unrecognised');

// An empty box is not an error. The recogniser runs on every
// keystroke; shouting at nothing yet typed is how a form becomes
// hostile.
{
  const r = recognise('   ', 'AuthKey_9F8B7C6D5E.p8');
  is('an empty secret raises no issue', r.issue, undefined);
  is('...and still reads the file name', r.keyId, '9F8B7C6D5E');
}

// --- the guide itself ---------------------------------------------
for (const s of PROVIDER_SPECS) {
  checked += 1;
  if (s.steps.length === 0) problems.push(`${s.provider}: no steps`);
  if (s.spec.length === 0) problems.push(`${s.provider}: nothing to recognise the file by`);
  for (const key of [s.title, ...s.steps, ...s.spec.map((r) => r.label)]) {
    if (!key.startsWith('push.')) problems.push(`${s.provider}: ${key} is not an i18n key`);
  }
  // The facts must NOT be i18n keys — a translated file name is a
  // file name nobody can find.
  for (const row of s.spec) {
    if (row.value.startsWith('push.')) problems.push(`${s.provider}: ${row.value} must be a literal`);
  }
}
// The twins are named by the thing that catches them, not by a
// warning in the guide — google-services.json by `recognise`, above,
// and the App Store Connect key by the probe's refusal, which
// `push_credential_probe.rs` asserts contains "App Store Connect".
// So what the guide must not do is grow a second copy: a warning read
// before the mistake is the weaker one, and it is the one someone
// reads first.
for (const s of PROVIDER_SPECS) {
  checked += 1;
  if ('notThis' in s) {
    problems.push(`${s.provider}: the guide is explaining a failure the machine already names`);
  }
}

// --- verdict colours ----------------------------------------------
is('ok reads as good', verdictTone('ok'), 'text-kind-probe');
is('limited is not the same colour as rejected', verdictTone('limited') === verdictTone('rejected'), false);
is('an unknown verdict is quiet', verdictTone(null), 'text-fg-subtle');

if (problems.length > 0) {
  console.error('✗ credential recognition:\n');
  for (const p of problems) console.error(`    ${p}`);
  process.exit(1);
}
console.log(`✓ ${checked} recognition assertions; the twins are named where they are caught`);
