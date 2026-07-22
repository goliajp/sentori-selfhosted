// Per-project releases — list deploys + per-release sourcemap /
// dsym / proguard artifact inventory.

import { useEffect, useState } from 'react';
import { useParams } from 'react-router-dom';

import { useT } from '../i18n';
import { api, ReleaseArtifact, ReleaseRow } from '../lib/api';
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
  Select,
  buttonClass,
  formatNumber,
  formatRelative,
} from '../components/ui';

export default function Releases() {
  const t = useT();
  const { id: projectId } = useParams<{ id: string }>();
  const [rows, setRows] = useState<ReleaseRow[]>([]);
  const [expanded, setExpanded] = useState<string | null>(null);
  const [artifacts, setArtifacts] = useState<Record<string, ReleaseArtifact[]>>(
    {},
  );
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showCreate, setShowCreate] = useState(false);
  const [newName, setNewName] = useState('');
  const [sdkToken, setSdkToken] = useState('');

  async function create() {
    if (!newName.trim() || !sdkToken.trim()) return;
    try {
      await api.createDeploy(
        { name: newName.trim(), deploy_at: new Date().toISOString() },
        sdkToken.trim(),
      );
      setNewName('');
      setSdkToken('');
      setShowCreate(false);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function refresh() {
    if (!projectId) return;
    setLoading(true);
    setError(null);
    try {
      const r = await api.listReleases(projectId);
      setRows(r.releases);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }
  useEffect(() => {
    refresh();
  }, [projectId]);

  async function expand(id: string) {
    if (!projectId) return;
    if (expanded === id) {
      setExpanded(null);
      return;
    }
    setExpanded(id);
    if (!artifacts[id]) await loadArtifacts(id);
  }

  async function loadArtifacts(id: string) {
    if (!projectId) return;
    try {
      const r = await api.listArtifacts(projectId, id);
      setArtifacts(a => ({ ...a, [id]: r.artifacts }));
    } catch (e) {
      setError(String(e));
    }
  }

  async function destroy(r: ReleaseRow) {
    if (!confirm(t('releases.confirmDelete').replace('{name}', r.name)))
      return;
    try {
      await api.deleteRelease(r.id);
      await refresh();
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
        title={t('releases.title')}
        subtitle={t('releases.subtitle')}
        actions={
          <Button onClick={() => setShowCreate(!showCreate)} size="sm">
            {showCreate ? t('action.cancel') : `+ ${t('releases.markShort')}`}
          </Button>
        }
      />

      {showCreate && (
        <Card>
          <CardHeader title={t('releases.mark')} />
          <CardBody>
            <p className="text-xs text-fg-subtle mb-2">
              Mints a release row via the public /v1/deploys endpoint.
              Requires a project SDK token (st_pk_...).
            </p>
            <input
              className="w-full rounded border border-border-strong bg-surface px-3 py-2 text-sm"
              placeholder={t('releases.namePlaceholder')}
              value={newName}
              onChange={e => setNewName(e.target.value)}
            />
            <input
              type="password"
              className="mt-2 w-full rounded border border-border-strong bg-surface px-3 py-2 text-sm font-mono"
              placeholder={t('releases.tokenPlaceholder')}
              value={sdkToken}
              onChange={e => setSdkToken(e.target.value)}
            />
            <div className="mt-2 flex gap-2">
              <Button onClick={create} size="sm">
                Mark deployed
              </Button>
            </div>
          </CardBody>
        </Card>
      )}
      {error && <ErrorBanner>{error}</ErrorBanner>}

      <Card>
        <CardHeader title={`${t('releases.title')} (${rows.length})`} />
        <CardBody>
          {loading ? (
            <div className="py-8 text-center text-sm text-fg-subtle">Loading…</div>
          ) : rows.length === 0 ? (
            <EmptyState
              title={t('releases.empty')}
              hint={t('releases.emptyHint')}
            />
          ) : (
            <div className="space-y-2">
              {rows.map(r => (
                <div
                  key={r.id}
                  className="rounded border border-border bg-white"
                >
                  <div className="flex items-center justify-between p-3">
                    <div className="flex items-center gap-3">
                      <button
                        onClick={() => expand(r.id)}
                        className="font-mono text-sm text-accent hover:underline"
                      >
                        {expanded === r.id ? '▼' : '▶'} {r.name}
                      </button>
                      {r.deploy_at && (
                        <Badge tone="ok">deployed</Badge>
                      )}
                    </div>
                    <div className="flex items-center gap-3">
                      <span className="text-xs text-fg-subtle">
                        {formatRelative(r.created_at)}
                      </span>
                      <Button
                        size="sm"
                        variant="danger"
                        onClick={() => destroy(r)}
                      >{t('action.delete')}</Button>
                    </div>
                  </div>
                  {expanded === r.id && (
                    <div className="border-t border-border p-3">
                      {/* The build that produced a release is the only
                          thing that has its symbols, so uploading is
                          attached to the release rather than living on
                          a settings page somewhere. */}
                      <ArtifactUpload
                        projectId={projectId}
                        releaseId={r.id}
                        onDone={() => {
                          setArtifacts(a => {
                            const next = { ...a };
                            delete next[r.id];
                            return next;
                          });
                          void loadArtifacts(r.id);
                        }}
                      />
                      {artifacts[r.id] ? (
                        artifacts[r.id].length === 0 ? (
                          <div className="py-2 text-center">
                            <p className="text-sm text-fg-muted">
                              {t('artifacts.none')}
                            </p>
                            <p className="mt-1 text-xs text-fg-subtle">
                              {t('artifacts.noneHint')}
                            </p>
                          </div>
                        ) : (
                          <DataTable
                            columns={[
                              { key: 'kind', label: t('artifacts.kind') },
                              { key: 'name', label: t('artifacts.name') },
                              { key: 'size', label: t('artifacts.size') },
                              { key: 'hash', label: t('artifacts.hash') },
                              { key: 'when', label: t('artifacts.uploaded') },
                            ]}
                            rows={artifacts[r.id].map(a => ({
                              key: a.id,
                              kind: <Badge>{a.kind}</Badge>,
                              name: a.name,
                              size: formatNumber(a.size_bytes),
                              hash: (
                                <span className="font-mono text-xs">
                                  {a.content_hash.slice(0, 12)}…
                                </span>
                              ),
                              when: formatRelative(a.created_at),
                            }))}
                          />
                        )
                      ) : (
                        <p className="text-xs text-fg-subtle">
                          {t('common.loading')}
                        </p>
                      )}
                    </div>
                  )}
                </div>
              ))}
            </div>
          )}
        </CardBody>
      </Card>
    </div>
  );
}

/**
 * Attach symbol files to a release.
 *
 * Kind is chosen rather than inferred from the extension: a `.map` is a
 * sourcemap and a `.txt` could be a proguard mapping or anything else,
 * and guessing wrong stores an artifact that silently never matches.
 */
function ArtifactUpload({
  projectId,
  releaseId,
  onDone,
}: {
  projectId: string;
  releaseId: string;
  onDone: () => void;
}) {
  const t = useT();
  const [kind, setKind] = useState<
    'sourcemap' | 'dsym' | 'proguard' | 'bundle'
  >('sourcemap');
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function pick(e: React.ChangeEvent<HTMLInputElement>) {
    const file = e.target.files?.[0];
    // Clear immediately so re-picking the same file fires again.
    e.target.value = '';
    if (!file) return;
    setBusy(true);
    setError(null);
    try {
      await api.uploadArtifact(projectId, releaseId, kind, file);
      onDone();
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="mb-3 flex items-center gap-2">
      <Select
        value={kind}
        disabled={busy}
        onChange={e => setKind(e.target.value as typeof kind)}
      >
        <option value="sourcemap">sourcemap</option>
        <option value="dsym">dSYM</option>
        <option value="proguard">proguard</option>
        <option value="bundle">bundle</option>
      </Select>
      <label className={buttonClass('secondary', 'md')}>
        {busy ? t('artifacts.uploading') : t('artifacts.upload')}
        <input type="file" className="hidden" disabled={busy} onChange={pick} />
      </label>
      {error && <span className="text-xs text-danger">{error}</span>}
    </div>
  );
}
