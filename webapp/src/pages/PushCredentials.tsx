// Push credentials admin — upsert / list / delete vendor secrets
// (APNs, FCM, WebPush, HCM, MiPush).

import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';

import { api, PushCredential } from '../lib/api';
import {
  Badge,
  Button,
  Card,
  CardHeader,
  DataTable,
  EmptyState,
  ErrorBanner,
  PageHeader,
  Section,
} from '../components/ui';

const PROVIDERS = ['apns', 'fcm', 'webpush', 'hcm', 'mipush'] as const;

export default function PushCredentials() {
  const { id: projectId } = useParams<{ id: string }>();
  const [rows, setRows] = useState<PushCredential[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [provider, setProvider] = useState<(typeof PROVIDERS)[number]>('apns');
  const [config, setConfig] = useState('{}');
  const [secret, setSecret] = useState('');
  const [showUpload, setShowUpload] = useState(false);

  async function refresh() {
    if (!projectId) return;
    setLoading(true);
    setError(null);
    try {
      const r = await api.listPushCredentials(projectId);
      setRows(r.credentials);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }
  useEffect(() => {
    refresh();
  }, [projectId]);

  async function upload() {
    if (!projectId) return;
    let parsed: unknown;
    try {
      parsed = JSON.parse(config);
    } catch {
      setError('Config must be valid JSON');
      return;
    }
    try {
      await api.upsertPushCredential(projectId, {
        provider,
        config: parsed,
        secret: secret || undefined,
      });
      setConfig('{}');
      setSecret('');
      setShowUpload(false);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function destroy(kind: string) {
    if (!projectId) return;
    if (!confirm(`Delete ${kind} credentials? Pending pushes will fail.`)) return;
    try {
      await api.deletePushCredential(projectId, kind);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  if (!projectId) {
    return <ErrorBanner>Project id missing</ErrorBanner>;
  }

  return (
    <div className="space-y-4">
      <PageHeader
        title="Push credentials"
        subtitle="Vendor secrets used by /v1/push/send. APNs p8, FCM service-account, WebPush VAPID, HCM/MiPush client secrets."
        actions={
          <div className="flex gap-2">
            <Button
              variant="secondary"
              onClick={() =>
                (window.location.href = `/projects/${projectId}/push-sends`)
              }
            >
              View sends
            </Button>
            <Button onClick={() => setShowUpload(true)}>+ Upload</Button>
          </div>
        }
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}
      <Card className="mb-2">
        <CardHeader title="Test push" />
        <Section>
          <p className="text-xs text-zinc-500 mb-2">
            Send a real test notification to a known device token to
            verify credentials + vendor adapter end-to-end.
          </p>
          <TestPushForm projectId={projectId} />
        </Section>
      </Card>

      {showUpload && (
        <Card>
          <CardHeader title="Upload credentials" />
          <Section>
            <label className="block text-xs text-zinc-500 mb-1">Provider</label>
            <select
              className="w-full rounded border border-zinc-300 px-3 py-2 text-sm"
              value={provider}
              onChange={e =>
                setProvider(e.target.value as (typeof PROVIDERS)[number])
              }
            >
              {PROVIDERS.map(p => (
                <option key={p} value={p}>
                  {p}
                </option>
              ))}
            </select>

            <label className="mt-3 block text-xs text-zinc-500 mb-1">
              Config (JSON — key id, team id, project id, vapid public key, …)
            </label>
            <textarea
              className="w-full h-32 rounded border border-zinc-300 px-3 py-2 text-xs font-mono"
              value={config}
              onChange={e => setConfig(e.target.value)}
            />

            <label className="mt-3 block text-xs text-zinc-500 mb-1">
              Secret (APNs p8 / FCM service-account json / VAPID private key)
            </label>
            <textarea
              className="w-full h-32 rounded border border-zinc-300 px-3 py-2 text-xs font-mono"
              value={secret}
              onChange={e => setSecret(e.target.value)}
              placeholder="-----BEGIN PRIVATE KEY-----\n..."
            />

            {provider === 'webpush' && (
              <div className="mt-3">
                <Button
                  variant="secondary"
                  size="sm"
                  onClick={async () => {
                    const out = await generateVapidKeypair();
                    setConfig(
                      JSON.stringify(
                        {
                          subject: 'mailto:admin@example.com',
                          vapidPublicKey: out.publicKeyB64url,
                        },
                        null,
                        2,
                      ),
                    );
                    setSecret(out.privatePem);
                  }}
                >
                  Generate VAPID keypair
                </Button>
                <p className="mt-1 text-[10px] text-zinc-500">
                  Browser-side WebCrypto. Public key goes into config
                  (for SDK), private PEM goes into secret. Never leaves
                  this browser → sent to server only on Save.
                </p>
              </div>
            )}

            <div className="mt-3 flex gap-2">
              <Button onClick={upload}>Save</Button>
              <Button variant="secondary" onClick={() => setShowUpload(false)}>
                Cancel
              </Button>
            </div>
          </Section>
        </Card>
      )}
      <Card>
        <CardHeader title={`Configured (${rows.length})`} />
        <Section>
          {loading ? (
            <div className="py-8 text-center text-sm text-zinc-500">
              Loading…
            </div>
          ) : rows.length === 0 ? (
            <EmptyState
              title="No credentials yet"
              hint="Upload at least one provider to start dispatching push."
            />
          ) : (
            <DataTable
              columns={[
                { key: 'kind', label: 'Provider' },
                { key: 'config', label: 'Config (no secret)' },
                { key: 'status', label: 'Last validate' },
                { key: 'actions', label: '' },
              ]}
              rows={rows.map(c => ({
                key: c.id,
                kind: <Badge>{c.kind}</Badge>,
                config: (
                  <code className="text-[10px] font-mono text-zinc-500">
                    {JSON.stringify(c.config).slice(0, 80)}
                  </code>
                ),
                status:
                  c.last_validate_status === 'ok' ? (
                    <Badge tone="ok">ok</Badge>
                  ) : c.last_validate_status ? (
                    <Badge tone="neutral">{c.last_validate_status}</Badge>
                  ) : (
                    <span className="text-xs text-zinc-400">never</span>
                  ),
                actions: (
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => destroy(c.kind)}
                  >
                    Delete
                  </Button>
                ),
              }))}
            />
          )}
        </Section>
      </Card>
    </div>
  );
}

/// Generate a VAPID-compatible EC P-256 keypair using Web Crypto.
/// Returns the public key as base64url uncompressed (65 bytes,
/// 0x04 || X || Y → 87 b64url chars) and the private key wrapped
/// in PEM-encoded PKCS8 (jsonwebtoken's expected input).
async function generateVapidKeypair(): Promise<{
  publicKeyB64url: string;
  privatePem: string;
}> {
  const { subtle } = window.crypto;
  const kp = await subtle.generateKey(
    { name: 'ECDSA', namedCurve: 'P-256' },
    true,
    ['sign', 'verify'],
  );
  // Public key: raw uncompressed point (0x04 || X || Y) — VAPID
  // wants this base64url-encoded.
  const rawPub = await subtle.exportKey('raw', kp.publicKey);
  const publicKeyB64url = bytesToB64url(new Uint8Array(rawPub));

  // Private key: PKCS8 DER → wrap in PEM.
  const pkcs8 = new Uint8Array(await subtle.exportKey('pkcs8', kp.privateKey));
  const b64 = btoa(String.fromCharCode(...pkcs8));
  const pemBody = b64.match(/.{1,64}/g)?.join('\n') ?? b64;
  const privatePem = `-----BEGIN PRIVATE KEY-----\n${pemBody}\n-----END PRIVATE KEY-----\n`;

  return { publicKeyB64url, privatePem };
}

function bytesToB64url(bytes: Uint8Array): string {
  return btoa(String.fromCharCode(...bytes))
    .replace(/\+/g, '-')
    .replace(/\//g, '_')
    .replace(/=+$/, '');
}

function TestPushForm({ projectId }: { projectId: string }) {
  const [tokenId, setTokenId] = useState('');
  const [title, setTitle] = useState('Sentori test');
  const [bodyText, setBodyText] = useState('hello from dashboard');
  const [msg, setMsg] = useState<string | null>(null);

  async function send() {
    if (!tokenId.trim()) return;
    setMsg(null);
    try {
      const r = await api.testPush(projectId, {
        deviceTokenId: tokenId.trim(),
        title,
        body: bodyText,
      });
      setMsg(`queued send_id=${r.send_id.slice(0, 8)}… provider=${r.provider}`);
    } catch (e) {
      setMsg(String(e).slice(0, 80));
    }
  }

  return (
    <div className="space-y-2">
      <input
        className="w-full rounded border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm font-mono"
        placeholder="device_token_id (UUID)"
        value={tokenId}
        onChange={e => setTokenId(e.target.value)}
      />
      <div className="grid grid-cols-2 gap-2">
        <input
          className="rounded border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm"
          placeholder="Title"
          value={title}
          onChange={e => setTitle(e.target.value)}
        />
        <input
          className="rounded border border-zinc-700 bg-zinc-900 px-3 py-2 text-sm"
          placeholder="Body"
          value={bodyText}
          onChange={e => setBodyText(e.target.value)}
        />
      </div>
      <div className="flex items-center gap-2">
        <Button onClick={send} size="sm">
          Send test
        </Button>
        {msg && (
          <span className="font-mono text-[10px] text-zinc-500">{msg}</span>
        )}
      </div>
    </div>
  );
}
