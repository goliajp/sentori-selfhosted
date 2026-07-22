// Workspace members + pending invites in one page.

import { useState } from 'react';

import { useT } from '../i18n';
import { api, InviteRow, MemberRow, Project } from '../lib/api';
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
  formatRelative,
} from '../components/ui';

export default function Members() {
  const t = useT();
  const [showInvite, setShowInvite] = useState(false);
  const [inviteEmail, setInviteEmail] = useState('');
  const [inviteRole, setInviteRole] = useState<'admin' | 'user'>('user');
  const [newInviteToken, setNewInviteToken] = useState<string | null>(null);
  // Which member's project access is open, and what it currently is.
  // Only the `user` role has any: owners and admins see every project,
  // and the server refuses a grant for them rather than pretending.
  const [accessFor, setAccessFor] = useState<MemberRow | null>(null);
  const [accessIds, setAccessIds] = useState<Set<string> | null>(null);

  const {
    data,
    loading,
    error,
    reload: refresh,
    setError,
  } = useAsyncData(
    async (): Promise<{
      members: MemberRow[];
      invites: InviteRow[];
      projects: Project[];
    }> => {
      const [m, i, p] = await Promise.all([
        api.listMembers(),
        api.listInvites(),
        api.listProjects(),
      ]);
      return { members: m.members, invites: i.invites, projects: p };
    },
    [],
    String,
  );
  const members = data?.members ?? [];
  const invites = data?.invites ?? [];
  const projects = data?.projects ?? [];

  // Access is stored per project, so reading one member's set means
  // asking every project who can see it. Fine at this scale, and it
  // keeps the server's shape honest rather than adding a per-user
  // endpoint that exists only for this screen.
  async function openAccess(m: MemberRow) {
    setAccessFor(m);
    setAccessIds(null);
    try {
      const lists = await Promise.all(
        projects.map(async p => ({
          id: p.id,
          users: (await api.listProjectAccess(p.id)).user_ids,
        })),
      );
      setAccessIds(
        new Set(lists.filter(l => l.users.includes(m.user_id)).map(l => l.id)),
      );
    } catch (e) {
      setError(String(e));
      setAccessFor(null);
    }
  }

  async function toggleAccess(projectId: string, on: boolean) {
    if (!accessFor) return;
    try {
      if (on) await api.grantProjectAccess(projectId, accessFor.user_id);
      else await api.revokeProjectAccess(projectId, accessFor.user_id);
      setAccessIds(prev => {
        const next = new Set(prev ?? []);
        if (on) next.add(projectId);
        else next.delete(projectId);
        return next;
      });
    } catch (e) {
      setError(String(e));
    }
  }

  async function setRole(uid: string, role: 'admin' | 'user') {
    try {
      await api.updateMemberRole(uid, role);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function removeMember(uid: string) {
    if (!confirm(t('members.confirmRemove'))) return;
    try {
      await api.removeMember(uid);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function mintInvite() {
    if (!inviteEmail) return;
    try {
      const r = await api.mintInvite({
        email: inviteEmail,
        role: inviteRole,
      });
      setNewInviteToken(r.token);
      setInviteEmail('');
      setShowInvite(false);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function revokeInvite(id: string) {
    if (!confirm(t('members.confirmRevokeInvite'))) return;
    try {
      await api.revokeInvite(id);
      refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="space-y-4">
      <PageHeader
        title={t('members.title')}
        subtitle={t('members.subtitle')}
        actions={<Button onClick={() => setShowInvite(true)}>{'+ ' + t('members.invite')}</Button>}
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {newInviteToken && (
        <Card>
          <CardHeader title={t('members.inviteLink')} />
          <CardBody>
            <pre className="overflow-x-auto whitespace-pre-wrap break-all bg-raised p-3 text-xs font-mono">
              {newInviteToken}
            </pre>
            <div className="mt-2">
              <Button onClick={() => setNewInviteToken(null)}>{t('action.done')}</Button>
            </div>
          </CardBody>
        </Card>
      )}

      {showInvite && (
        <Card>
          <CardHeader title={t('members.invite')} />
          <CardBody>
            <input
              className="h-8 w-full rounded border border-border px-2.5 text-sm"
              placeholder={t('auth.email')}
              value={inviteEmail}
              onChange={e => setInviteEmail(e.target.value)}
            />
            <select
              className="mt-2 w-full rounded border border-border px-3 py-2 text-sm"
              value={inviteRole}
              onChange={e => setInviteRole(e.target.value as 'admin' | 'user')}
            >
              <option value="user">user</option>
              <option value="admin">admin</option>
            </select>
            <div className="mt-2 flex gap-2">
              <Button onClick={mintInvite}>{t('members.sendInvite')}</Button>
              <Button variant="secondary" onClick={() => setShowInvite(false)}>{t('action.cancel')}</Button>
            </div>
          </CardBody>
        </Card>
      )}

      {accessFor && (
        <Card>
          <CardHeader
            title={t('members.accessFor').replace(
              '{who}',
              accessFor.email ?? t('members.unknownUser'),
            )}
            action={
              <Button variant="secondary" onClick={() => setAccessFor(null)}>
                {t('action.done')}
              </Button>
            }
          />
          <CardBody>
            {accessIds === null ? (
              <div className="py-4 text-sm text-fg-subtle">{t('common.loading')}</div>
            ) : projects.length === 0 ? (
              <EmptyState
                title={t('members.noProjectsToGrant')}
                hint={t('members.noProjectsToGrantHint')}
              />
            ) : (
              <div className="space-y-2">
                {projects.map(p => (
                  <label
                    key={p.id}
                    className="flex cursor-pointer items-center gap-2 text-sm"
                  >
                    <input
                      type="checkbox"
                      checked={accessIds.has(p.id)}
                      onChange={e => toggleAccess(p.id, e.target.checked)}
                    />
                    <span className="text-fg">{p.name}</span>
                    <span className="font-mono text-xs text-fg-subtle">{p.slug}</span>
                  </label>
                ))}
              </div>
            )}
            <p className="mt-3 text-xs text-fg-subtle">
              {t('members.accessHint')}
            </p>
          </CardBody>
        </Card>
      )}

      <Card>
        <CardHeader title={`${t('members.activeMembers')} (${members.length})`} />
        <CardBody>
          {loading ? (
            <div className="py-8 text-center text-sm text-fg-subtle">
              Loading…
            </div>
          ) : members.length === 0 ? (
            <EmptyState title={t('members.empty')} hint={t('members.emptyHint')} />
          ) : (
            <DataTable
              columns={[
                { key: 'uid', label: t('members.user') },
                { key: 'role', label: t('members.role') },
                { key: 'access', label: t('members.projectAccess') },
                { key: 'added', label: t('members.added') },
                { key: 'actions', label: '' },
              ]}
              rows={members.map(m => ({
                key: m.user_id,
                // The email, with who added them underneath. The uuid is
                // the join key, not a name — it belongs in the row's
                // React key and in copy-to-clipboard, not on screen.
                uid: (
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="truncate text-fg">
                        {m.email ?? t('members.unknownUser')}
                      </span>
                      {m.email && !m.email_verified && (
                        <Badge tone="warn">{t('members.unverified')}</Badge>
                      )}
                    </div>
                    {m.added_by_email && (
                      <div className="mt-0.5 text-xs text-fg-subtle">
                        {t('members.addedBy').replace('{who}', m.added_by_email)}
                      </div>
                    )}
                  </div>
                ),
                role: (
                  <Badge tone={m.role === 'owner' ? 'ok' : 'neutral'}>
                    {m.role}
                  </Badge>
                ),
                // Only the `user` role has a set to manage. For owners
                // and admins the honest answer is not an empty list but
                // "all of them", which no control should invite you to
                // edit.
                access:
                  m.role === 'user' ? (
                    <Button
                      size="sm"
                      variant="secondary"
                      onClick={() => openAccess(m)}
                    >
                      {t('members.manageAccess')}
                    </Button>
                  ) : (
                    <span className="text-xs text-fg-subtle">
                      {t('members.allProjects')}
                    </span>
                  ),
                added: formatRelative(m.added_at),
                actions:
                  m.role !== 'owner' ? (
                    <div className="flex gap-1">
                      <Button
                        size="sm"
                        variant="secondary"
                        onClick={() =>
                          setRole(m.user_id, m.role === 'admin' ? 'user' : 'admin')
                        }
                      >
                        {m.role === 'admin' ? '→ user' : '→ admin'}
                      </Button>
                      <Button
                        size="sm"
                        variant="danger"
                        onClick={() => removeMember(m.user_id)}
                      >{t('action.remove')}</Button>
                    </div>
                  ) : null,
              }))}
            />
          )}
        </CardBody>
      </Card>

      <Card>
        <CardHeader title={`${t('members.invites')} (${invites.length})`} />
        <CardBody>
          {invites.length === 0 ? (
            <EmptyState
              title={t('members.noInvites')}
              hint={t('members.invitesHint')}
            />
          ) : (
            <DataTable
              columns={[
                { key: 'email', label: 'Email' },
                { key: 'role', label: 'Role' },
                { key: 'created', label: 'Sent' },
                { key: 'status', label: 'Status' },
                { key: 'actions', label: '' },
              ]}
              rows={invites.map(i => ({
                key: i.id,
                email: i.email,
                role: <Badge>{i.role}</Badge>,
                created: formatRelative(i.created_at),
                status: i.accepted_at ? (
                  <Badge tone="ok">accepted</Badge>
                ) : new Date(i.expires_at) < new Date() ? (
                  <Badge tone="neutral">expired</Badge>
                ) : (
                  <Badge tone="neutral">pending</Badge>
                ),
                actions:
                  !i.accepted_at && new Date(i.expires_at) > new Date() ? (
                    <Button
                      size="sm"
                      variant="danger"
                      onClick={() => revokeInvite(i.id)}
                    >{t('action.revoke')}</Button>
                  ) : null,
              }))}
            />
          )}
        </CardBody>
      </Card>
    </div>
  );
}
