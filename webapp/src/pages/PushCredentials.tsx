// Push credentials admin — upsert / list / delete vendor secrets
// (APNs, FCM, WebPush, HCM, MiPush).

import { useState } from 'react';
import { useParams } from 'react-router-dom';

import { useT } from '../i18n';
import { api, PushCredential } from '../lib/api';
import { useAsyncData } from '../lib/useAsyncData';
import {
  Badge,
  Button,
  Card,
  CardBody,
  CardHeader,
  DataTable,
  EmptyState,
  ErrorBanner,
  PageHeader,
} from '../components/ui';

const PROVIDERS = ['apns', 'fcm', 'webpush', 'hcm', 'mipush'] as const;

export default function PushCredentials() {
  const t = useT();
  const { id: projectId } = useParams<{ id: string }>();
  const [provider, setProvider] = useState<(typeof PROVIDERS)[number]>('apns');
  const [config, setConfig] = useState('{}');
  const [secret, setSecret] = useState('');
  const [showUpload, setShowUpload] = useState(false);

  const {
    data,
    loading,
    error,
    reload: refresh,
    setError,
  } = useAsyncData(
    async (): Promise<PushCredential[]> =>
      projectId ? (await api.listPushCredentials(projectId)).credentials : [],
    [projectId],
    String,
  );
  const rows = data ?? [];

  async function upload() {
    if (!projectId) return;
    let parsed: unknown;
    try {
      parsed = JSON.parse(config);
    } catch {
      setError(t('common.jsonInvalid'));
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
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function destroy(kind: string) {
    if (!projectId) return;
    if (!confirm(`Delete ${kind} credentials? Pending pushes will fail.`)) return;
    try {
      await api.deletePushCredential(projectId, kind);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  if (!projectId) {
    return <ErrorBanner>{t('common.missingProjectId')}</ErrorBanner>;
  }

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('push.credentials')}
        subtitle={t('push.credSubtitle')}
        actions={
          <div className="flex gap-2">
            <Button
              variant="secondary"
              onClick={() =>
                (window.location.href = `/projects/${projectId}/push-sends`)
              }
            >{t('push.viewSends')}</Button>
            <Button onClick={() => setShowUpload(true)}>{'+ ' + t('push.upload')}</Button>
          </div>
        }
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}
      <Card className="mb-2">
        <CardHeader title={t('push.test')} />
        <CardBody>
          <p className="text-xs text-fg-subtle mb-2">
            Send a real test notification to a known device token to
            verify credentials + vendor adapter end-to-end.
          </p>
          <TestPushForm projectId={projectId} />
        </CardBody>
      </Card>

      {showUpload && (
        <Card>
          <CardHeader title={t('push.upload')} />
          <CardBody>
            <label className="block text-xs text-fg-subtle mb-1">Provider</label>
            <select
              className="h-8 w-full rounded border border-border px-2.5 text-sm"
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

            <label className="mt-3 block text-xs text-fg-subtle mb-1">
              Config (JSON — key id, team id, project id, vapid public key, …)
            </label>
            <textarea
              className="w-full h-32 rounded border border-border px-3 py-2 text-xs font-mono"
              value={config}
              onChange={e => setConfig(e.target.value)}
            />

            <label className="mt-3 block text-xs text-fg-subtle mb-1">
              Secret (APNs p8 / FCM service-account json / VAPID private key)
            </label>
            <textarea
              className="w-full h-32 rounded border border-border px-3 py-2 text-xs font-mono"
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
                >{t('push.generateVapid')}</Button>
                <p className="mt-1 text-xs text-fg-subtle">
                  Browser-side WebCrypto. Public key goes into config
                  (for SDK), private PEM goes into secret. Never leaves
                  this browser → sent to server only on Save.
                </p>
              </div>
            )}

            <div className="mt-3 flex gap-2">
              <Button onClick={upload}>{t('action.save')}</Button>
              <Button variant="secondary" onClick={() => setShowUpload(false)}>{t('action.cancel')}</Button>
            </div>
          </CardBody>
        </Card>
      )}
      <Card>
        <CardHeader title={`${t('common.configured')} (${rows.length})`} />
        <CardBody>
          {loading ? (
            <div className="py-8 text-center text-sm text-fg-subtle">
              Loading…
            </div>
          ) : rows.length === 0 ? (
            <EmptyState
              title={t('push.empty')}
              hint={t('push.emptyHint')}
            />
          ) : (
            <DataTable
              columns={[
                { key: 'kind', label: t('push.provider') },
                { key: 'config', label: t('push.config') },
                { key: 'status', label: t('push.lastValidate') },
                { key: 'actions', label: '' },
              ]}
              rows={rows.map(c => ({
                key: c.id,
                kind: <Badge>{c.kind}</Badge>,
                config: (
                  <code className="text-xs font-mono text-fg-subtle">
                    {JSON.stringify(c.config).slice(0, 80)}
                  </code>
                ),
                status:
                  c.last_validate_status === 'ok' ? (
                    <Badge tone="ok">ok</Badge>
                  ) : c.last_validate_status ? (
                    <Badge tone="neutral">{c.last_validate_status}</Badge>
                  ) : (
                    <span className="text-xs text-fg-muted">never</span>
                  ),
                actions: (
                  <Button
                    size="sm"
                    variant="danger"
                    onClick={() => destroy(c.kind)}
                  >{t('action.delete')}</Button>
                ),
              }))}
            />
          )}
        </CardBody>
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
  const t = useT();
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
        className="w-full rounded border border-border-strong bg-surface px-3 py-2 text-sm font-mono"
        placeholder="device_token_id (UUID)"
        value={tokenId}
        onChange={e => setTokenId(e.target.value)}
      />
      <div className="grid grid-cols-2 gap-2">
        <input
          className="rounded border border-border-strong bg-surface px-3 py-2 text-sm"
          placeholder={t('push.notifTitle')}
          value={title}
          onChange={e => setTitle(e.target.value)}
        />
        <input
          className="rounded border border-border-strong bg-surface px-3 py-2 text-sm"
          placeholder={t('push.notifBody')}
          value={bodyText}
          onChange={e => setBodyText(e.target.value)}
        />
      </div>
      <div className="flex items-center gap-2">
        <Button onClick={send} size="sm">{t('push.sendTest')}</Button>
        {msg && (
          <span className="font-mono text-xs text-fg-subtle">{msg}</span>
        )}
      </div>
    </div>
  );
}
