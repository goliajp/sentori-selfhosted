// Workspace members + pending invites in one page.

import { useEffect, useState } from 'react';

import { api, InviteRow, MemberRow } from '../lib/api';
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
  formatRelative,
} from '../components/ui';

export default function Members() {
  const [members, setMembers] = useState<MemberRow[]>([]);
  const [invites, setInvites] = useState<InviteRow[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showInvite, setShowInvite] = useState(false);
  const [inviteEmail, setInviteEmail] = useState('');
  const [inviteRole, setInviteRole] = useState<'admin' | 'user'>('user');
  const [invitedBy, setInvitedBy] = useState(
    typeof localStorage !== 'undefined'
      ? localStorage.getItem('sentori_user_id') ?? ''
      : '',
  );
  const [newInviteToken, setNewInviteToken] = useState<string | null>(null);

  async function refresh() {
    setLoading(true);
    setError(null);
    try {
      const [m, i] = await Promise.all([api.listMembers(), api.listInvites()]);
      setMembers(m.members);
      setInvites(i.invites);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  }
  useEffect(() => {
    refresh();
  }, []);

  async function setRole(uid: string, role: 'admin' | 'user') {
    try {
      await api.updateMemberRole(uid, role);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function removeMember(uid: string) {
    if (!confirm('Remove this member from the workspace?')) return;
    try {
      await api.removeMember(uid);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function mintInvite() {
    if (!inviteEmail || !invitedBy) return;
    try {
      const r = await api.mintInvite({
        email: inviteEmail,
        role: inviteRole,
        invited_by: invitedBy,
      });
      setNewInviteToken(r.token);
      setInviteEmail('');
      setShowInvite(false);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  async function revokeInvite(id: string) {
    if (!confirm('Revoke this invite?')) return;
    try {
      await api.revokeInvite(id);
      await refresh();
    } catch (e) {
      setError(String(e));
    }
  }

  return (
    <div className="space-y-4">
      <PageHeader
        title="Members"
        subtitle="Workspace owner / admin / user roles + pending invites."
        actions={<Button onClick={() => setShowInvite(true)}>+ Invite</Button>}
      />
      {error && <ErrorBanner>{error}</ErrorBanner>}

      {newInviteToken && (
        <Card>
          <CardHeader title="Invite link (copy now — shown once)" />
          <Section>
            <pre className="overflow-x-auto whitespace-pre-wrap break-all bg-zinc-50 p-3 text-xs font-mono">
              {newInviteToken}
            </pre>
            <div className="mt-2">
              <Button onClick={() => setNewInviteToken(null)}>Done</Button>
            </div>
          </Section>
        </Card>
      )}

      {showInvite && (
        <Card>
          <CardHeader title="Invite member" />
          <Section>
            <input
              className="w-full rounded border border-zinc-300 px-3 py-2 text-sm"
              placeholder="Email"
              value={inviteEmail}
              onChange={e => setInviteEmail(e.target.value)}
            />
            <select
              className="mt-2 w-full rounded border border-zinc-300 px-3 py-2 text-sm"
              value={inviteRole}
              onChange={e => setInviteRole(e.target.value as 'admin' | 'user')}
            >
              <option value="user">user</option>
              <option value="admin">admin</option>
            </select>
            <input
              className="mt-2 w-full rounded border border-zinc-300 px-3 py-2 text-sm font-mono"
              placeholder="Inviter user_id (UUID — yours)"
              value={invitedBy}
              onChange={e => setInvitedBy(e.target.value)}
            />
            <div className="mt-2 flex gap-2">
              <Button onClick={mintInvite}>Send invite</Button>
              <Button variant="secondary" onClick={() => setShowInvite(false)}>
                Cancel
              </Button>
            </div>
          </Section>
        </Card>
      )}

      <Card>
        <CardHeader title={`Active members (${members.length})`} />
        <Section>
          {loading ? (
            <div className="py-8 text-center text-sm text-zinc-500">
              Loading…
            </div>
          ) : members.length === 0 ? (
            <EmptyState title="No members" hint="Invite teammates to start." />
          ) : (
            <DataTable
              columns={[
                { key: 'uid', label: 'User' },
                { key: 'role', label: 'Role' },
                { key: 'added', label: 'Added' },
                { key: 'actions', label: '' },
              ]}
              rows={members.map(m => ({
                key: m.user_id,
                uid: (
                  <span className="font-mono text-xs">{m.user_id}</span>
                ),
                role: (
                  <Badge tone={m.role === 'owner' ? 'ok' : 'neutral'}>
                    {m.role}
                  </Badge>
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
                      >
                        Remove
                      </Button>
                    </div>
                  ) : null,
              }))}
            />
          )}
        </Section>
      </Card>

      <Card>
        <CardHeader title={`Invites (${invites.length})`} />
        <Section>
          {invites.length === 0 ? (
            <EmptyState
              title="No invites"
              hint="Pending and historical invites land here."
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
                    >
                      Revoke
                    </Button>
                  ) : null,
              }))}
            />
          )}
        </Section>
      </Card>
    </div>
  );
}
