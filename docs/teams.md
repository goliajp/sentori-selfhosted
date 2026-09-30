# Teams, projects, ownership

Sentori organizations are flat by default — every member sees every
project. Teams turn that into role-scoped access: bind a project to one
or more teams and only members of those teams (plus org owner / admin)
can read it.

This guide walks through the full lifecycle: when to use teams, how
they interact with project access, ownership transfer, and the audit
log that records every admin action.

## Vocabulary in one paragraph

An **org** owns **projects** and has **members** with one of three roles:
`owner` (one per org, can delete + transfer), `admin` (everything except
those two), `member` (read-only on org metadata, can self-leave). Inside
the org you can carve out **teams** with their own membership lists; a
team's role is `lead` or `member`. **Project ↔ team binding** is
many-to-many: a project can belong to several teams; a team can own
several projects. A project with no team binding stays open to every
org member.

## When to use a team

You don't need teams while a single small group is using the org —
flat membership works fine. Spin up a team when:

- You want to **separate stages of one product** (e.g. `mobile` team
  sees the iOS / Android crash projects but not the back-office
  internal tools).
- You're **bringing in contractors** who should only see their slice.
- You want to **delegate member management** without giving full
  org-admin: a team lead can add / remove members on their team but
  can't touch the rest of the org.

If none of those apply, skip teams — pre-binding projects without a
clear reason just adds clicks.

## Creating a team

1. Open `Teams` in the org nav (visible to every member).
2. Org admins / owners see a **Create team** form. Slug is
   `[a-z0-9-]{3,32}` — short, machine-stable, the same shape as org
   slugs. Name is the human label; description is free-form.
3. The new team starts empty. Add members from the team detail page;
   any current org member is eligible.

Team members can see the team and its bound projects. **Team leads**
can additionally add / remove other team members.

## Binding a project to one or more teams

A project is "open" to the whole org until it has a team binding. Once
bound, **only members of the bound team(s)** plus org owner / admin can
read or write.

Two routes lead to the same UI:

- From the project: *project → settings → Team access*
- From the team: *teams → \<team\> → Projects → Bind project*

Both write to the same `project_teams` join table. Add several teams
to one project if you need broader access (e.g. `mobile` and
`oncall` both touching the iOS error project).

The dashboard's project list, issues, and tokens all respect the
binding — non-team members get `403 projectNotInTeam` from the API
and the project disappears from their org view entirely. Org admin
always bypasses the gate.

## Inviting someone directly into a team

Skip the "invite, then add to team" two-step: the invite form shows a
team dropdown when the org has teams. Picking one stamps the invite
with that team, so the recipient gets both the org membership and the
team membership in a single transaction on accept.

If the team is deleted while the invite is pending, the recipient still
joins the org — the invite gracefully degrades to an org-only invite
rather than failing on accept.

## Transferring ownership

Owner is a one-of-a-kind role: there's exactly one per org and only
the owner can delete the org or initiate further transfers. To hand
the org over:

1. Promote the target to **admin** if they aren't already (member is
   not eligible to receive ownership).
2. Open *Settings → Transfer ownership* (owner-only, dashed danger
   border).
3. Pick the new owner from the eligible-admins dropdown.
4. **Type the org slug** in the confirm input. The submit button stays
   disabled until the slug matches.
5. The new owner gets a confirmation email with a 7-day link.

The transfer **only completes when the new owner clicks the email link
and confirms.** Until then, no role changes — you can revoke by
ignoring the email or, if you change your mind, just initiate a new
transfer (the old token stays valid until expiry but the new owner
sees both invites if you don't).

When the transfer completes:

- The new owner becomes `owner`.
- The old owner is demoted to `admin` (still has full management
  rights except `org.delete` / `transfer.initiate`).
- Both users get an email; the old owner's note explicitly says "if
  this wasn't you, contact support".
- Both events land in the audit log: `org.transfer.requested` and
  `org.transfer.accepted`.

## The audit log

Every admin-level mutating action lands in the **audit log**, viewable
by org owners / admins under *Audit* in the nav. Coverage today:

- Org create / patch
- Member role change / removal
- Team create / delete / member add / member remove
- Project create
- Project ↔ team bind / unbind
- Token create / revoke
- Ownership transfer requested / accepted

Each row records the actor's email, action, target, timestamp, and a
JSON payload with the relevant context (the new role, the team slug
that was bound, etc.). Filter by action / actor / time, paginate
through history with the cursor button, or **Export CSV** to get the
current view.

The audit log is append-only — there's no UI to redact rows. A future
release will move the table out of the org-delete cascade so a deleted
org's history stays queryable for a retention period.

## Reference

- API: `/api/orgs/{slug}/teams[/...]` and
  `/admin/api/projects/{id}/teams/{team_slug}` — see the
  [protocol reference](./protocol.md).
- Dashboard route: `/org/{slug}/teams`, `/org/{slug}/teams/{team_slug}`,
  `/org/{slug}/audit`, `/org/{slug}/projects/{id}/settings/teams`.
- Server-side schema lives in migration
  `0010_phase18_orgs.sql` (teams / project_teams / audit_logs /
  org_ownership_transfers) and `0011_invite_team.sql` (invite team
  binding).
