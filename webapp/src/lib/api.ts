import { eventFilterParams, type EventFilters } from './event-filters';
import type { ProjectTransfer, TransferCreate, TransferCommand, TransferTail, TransferUpload } from './transfer-contract';
export type CloudUsage = {
  limits: { monthlyEvents: number; monthlyReplays: number; monthlyIngestBytes: number; artifactBytes: number; projects: number; eventRetentionDays: number; replayRetentionDays: number; eventsPerSecond: number };
  active: boolean; checkedAt: string; validUntil: string; paidUntil: string | null; month: string; resetsAt: string;
  events: number; replays: number; ingestBytes: number; artifactBytes: number; projects: number;
};
import type { InstanceOperation, BackupCommand, ResetPreparation, ResetPermit } from './system-contract';
import type { PushCommand, PushCommandResult, PushDetail } from './push-contract';
import type { Task, TaskCommand, TaskEvent, InboxItem, InboxPreferences } from './team-contract';
import type { Monitor, MonitorCommand, MonitorEvent } from './monitor-contract';
import type { Integration, IntegrationKind, IntegrationConfigure, IntegrationSend, IntegrationRetry, Delivery } from './integration-contract';
export interface JobRow {
  id: string; projectId: string; releaseId: string; kind: 'resymbolicate';
  state: 'queued' | 'running' | 'retry_wait' | 'succeeded' | 'failed' | 'cancelled';
  attempt: number; maxAttempts: number; cancelRequested: boolean;
  scanned: number; rewritten: number; frames: number; scanLimit: number;
  errorCode: string | null; createdAt: string; updatedAt: string; finishedAt: string | null;
}

import type { InstanceCheck, InstanceHealth, InstanceInfo, SystemInfo } from "./system-contract";
// The dashboard's typed fetch client — one class, one method per
// server route, same-origin cookies. The surface mirrors
// self-hosted/server/src/handlers exactly; if a method has no
// server route, it does not belong here.

export type Role = 'admin' | 'superadmin';

export type Me = {
  userId: string;
  email: string;
  displayName: string | null;
  role: Role;
};

export type Project = {
  id: string;
  name: string;
  platform: string | null;
  version: number;
  createdAt: string;
};

export type IssueSummary = {
  id: string;
  projectId: string;
  kind: 'assert' | 'error' | 'probe' | 'trace' | 'warn';
  title: string;
  messageSample: string;
  surface: Record<string, unknown>;
  status: 'ignored' | 'open' | 'resolved';
  firstSeen: string;
  lastSeen: string;
  /** When we received it. `lastSeen` is the device's own clock. */
  lastReceivedAt?: string | null;
  eventCount: number;
  usersCount: number;
  maxPerUser: number;
  lastRelease: string;
  assigneeUserId: string | null;
  resolvedAt: string | null;
  resolvedInRelease: string | null;
  regressedAt: string | null;
  regressedInRelease: string | null;
  /** NULL on pre-split historical issues (aggregated across
   *  environments/platforms before server 2.9). */
  environment?: string | null;
  platform?: string | null;
};

export type IssueReleaseRow = {
  release: string;
  events: number;
  firstAt: string;
  lastAt: string;
};

export type IssueActivity = {
  id: string;
  at: string;
  kind: 'assign' | 'note' | 'regression' | 'status';
  body: Record<string, unknown>;
  actorUserId: string | null;
  actorEmail: string | null;
};

export type IssueDetail = IssueSummary & {
  activity: IssueActivity[];
  assignees: { id: string; email: string; displayName: string | null }[];
  releases?: IssueReleaseRow[];
};

export type OccurrenceRow = {
  id: string;
  kind: string;
  platform: string;
  occurredAt: string;
  receivedAt: string;
  release: string;
  environment: string;
  userKey: string | null;
  /** Ref of this event's `screens` attachment, when it has one —
   *  lets the replay dock fall back to the newest occurrence that
   *  actually captured pixels. */
  screensRef: string | null;
};

export type AttachmentRow = {
  ref: string;
  kind: string;
  mediaType: string;
  sizeBytes: number;
  capturedAt: string;
};

export type EventDetail = {
  id: string;
  projectId: string;
  issueId: string;
  kind: string;
  platform: string;
  occurredAt: string;
  receivedAt: string;
  release: string;
  environment: string;
  userKey: string | null;
  payload: Record<string, unknown>;
  attachments: AttachmentRow[];
};

export type ArtifactSummary = {kind: string; count: number; usable: boolean; broken: boolean; unchecked: boolean}[];
export type ProjectApp = {
  id: string; projectId: string; platform: string; kind: string; identifier: string; name: string | null;
  identitySource: 'configured' | 'reported' | 'release' | 'unknown';
  repositoryUrl: string | null; version: number; createdAt: string;
  lastEventAt: string | null; lastSessionAt: string | null; events24h: number; errors24h: number;
  releaseCount: number; latestRelease: {id: string; name: string; createdAt: string} | null;
  artifactSummary: ArtifactSummary;
};

export type ProjectHealth = {
  apps: ProjectApp[];
  artifactSummary: ArtifactSummary;
  /** Null until the SDK carries a backendHealthUrl for the project. */
  backend: null | {
    url: string;
    lastOk: boolean | null;
    lastStatus: number | null;
    lastLatencyMs: number | null;
    lastCheckedAt: null | string;
    checks24h: number;
    ok24h: number;
  };
  lastEventAt: null | string;
  counts24h: Record<string, number>;
  users24h: number;
  platforms24h: Record<string, number>;
  latestRelease: null | string;
  latestReleaseArtifacts: string[];
  replay24h: { eligible: number; withScreens: number };
};

export type ContextEventRow = {
  id: string;
  issueId: string;
  kind: IssueSummary['kind'];
  name: string;
  occurredAt: string;
};

export type TokenRow = {
  id: string;
  name: string;
  scope: 'api' | 'ingest';
  last4: string | null;
  createdAt: string;
  revokedAt: string | null;
};

export type UserRow = {
  id: string;
  email: string;
  role: Role;
  displayName: string | null;
  createdAt: string;
  lastLoginAt: string | null;
  projects: string[];
};

export type ReleaseRow = {
  apps?: Pick<ProjectApp, 'id' | 'name' | 'identifier' | 'platform'>[];
  id: string;
  name: string;
  created_at?: string;
  createdAt?: string;
  /** Platforms that actually reported events in this release — a
   *  missing artifact only matters for a platform that is sending. */
  platforms?: string[];
  /** Bounded status summary for the list; artifact metadata loads on opening a release. */
  artifactSummary?: {kind: string; count: number; usable: boolean; broken: boolean; unchecked: boolean}[];
};

export type ArtifactRow = {
  id: string;
  kind: string;
  name: string;
  content_hash?: string;
  size_bytes?: number;
  created_at?: string;
  /** Parsed at upload. `null`/absent on artifacts stored before the
   *  check existed — never looked at, which is not the same claim as
   *  looked at and fine. `false` symbolicates nothing. */
  usable?: boolean | null;
};

export type AuditRow = {
  id: string;
  projectId: string | null;
  actorUserId: string | null;
  actorEmail: string | null;
  action: string;
  targetType: string | null;
  targetId: string | null;
  payload: Record<string, unknown>;
  createdAt: string;
};

/** A provider credential as the dashboard sees it — never the secret
 *  itself, only whether one is there and whether it last worked. */
export type PushCredential = {
  encrypted: boolean;
  version: number;
  id: string;
  kind: string;
  /** Redacted shape from the server; the secret never leaves it. */
  config: Record<string, unknown>;
  created_at?: string;
  last_validated_at?: null | string;
  last_validate_status?: null | string;
  /** Why the worker could not use this credential, from the same
   *  check the save runs. Null when it is fine. A credential stored
   *  before anything was checked is still stored, and used to
   *  announce itself only as a notification that never arrived. */
  /** Why the worker cannot use it, as a code the console says in its
   *  own language. It used to be an English sentence printed under a
   *  Chinese label. */
  problem?: null | { code: string; field: null | string };
  /** Whether the send path uses this one. Exactly one per kind. */
  active?: boolean;
  /** What the operator calls it — two Apple teams are otherwise two
   *  identical rows. */
  label?: null | string;
  /** The vendor's own words behind `last_validate_status`. A verdict
   *  of `limited` with no reason is worse than no verdict. */
  last_validate_detail?: null | string;
};

/** What the vendor said when we asked. Three states, not two:
 *  `limited` means the credential is real and still cannot do this
 *  job, which is a different repair from `rejected`. */
export type ProbeVerdict = {
  status: 'limited' | 'not_implemented' | 'ok' | 'rejected' | 'unreachable';
  code: null | string;
  field: null | string;
  detail: null | string;
  safeToActivate: boolean;
};

export type PushSend = {
  displayState: string;
  credentialId: string | null;
  deliveredAt: string | null;
  version: number;
  id: string;
  token_id: string;
  provider: string;
  status: string;
  provider_outcome?: null | string;
  error?: null | string;
  retry_count: number;
  /** What was sent. The table listed outcomes and never said which
   *  notification they belonged to, so a failed row could not be
   *  matched to the send that produced it. */
  payload?: { body?: string; title?: string } & Record<string, unknown>;
  created_at: string;
  sent_at?: null | string;
  next_attempt_at?: null | string;
};

/** One thing that is set up, missing, or worth knowing.
 *
 *  `blocked` means nothing can arrive until it is dealt with; `warn`
 *  means it can send but some way of aiming reaches nobody; `info` is
 *  a fact, not a fault. */
export type PushCheck = {
  data: Record<string, unknown>;
  id: string;
  level: 'blocked' | 'info' | 'warn';
};

export type PushHealth = {
  sent24h: number;
  failed24h: number;
  queued: number;
  lastSendAt: null | string;
  liveTokens: number;
  quarantinedTokens: number;
  /** Devices carrying a user key — the ones an issue can address.
   *  The rest can only be broadcast to. */
  identifiedTokens: number;
  /** Why the failures failed. "12 failed" is an alarm; "12 failed,
   *  BadDeviceToken" is a fix. */
  reasons: { reason: string; count: number }[];
};

export type PushDevice = {
  version: number;
  id: string;
  provider: string;
  env: null | string;
  /** What the host passed to `sentori.push.register({ metadata })`,
   *  verbatim. `{}` means it sent none — which is the answer to
   *  "did my metadata arrive", and until server 2.22.0 it was `{}`
   *  for every device because nothing carried the field. */
  metadata: Record<string, unknown>;
  /** `sentori.user()` ran before `register()`. Without it the device
   *  gets broadcasts and cannot be reached from an issue. */
  addressable: boolean;
  badStreak: number;
  revokedAt: null | string;
  lastSeenAt: string;
  createdAt: string;
  /** Last six characters. Enough to tell two devices apart, not
   *  enough to push to one. */
  tokenTail: null | string;
  /** What `sentori.user()` was told about the person — plan, cohort,
   *  org. Selectable by a send, unlike the identity itself, which is
   *  only ever a hash. `{}` until the host passes any. */
  traits?: Record<string, unknown>;
  /** The last six of the identity key, when one is known. The
   *  column used to show only a yes/no called "addressable", which
   *  asks whether `sentori.user()` ran before `register()` — not what
   *  the word suggests, and two readers went to the source to find
   *  out. A key or a dash needs no explaining. */
  userKeyTail?: null | string;
};

/** One of the three ways to name an audience.
 *
 *  They are not three features: `appUserId` is one leaf of an
 *  expression and `traits` is a conjunction of them, so the server
 *  compiles all three into the same query. Exactly one is set.
 */
export type AudienceRequest = {
  appUserId?: string;
  audience?: unknown;
  traits?: Record<string, unknown>;
};

/** A device the audience selected, with why it did.
 *
 *  Never the push token — the device list refuses to hand one back
 *  and a preview must not become the way around that. */
export type AudienceSample = {
  addressable: boolean;
  id: string;
  metadata: Record<string, unknown>;
  provider: string;
  traits: Record<string, unknown>;
  userKeyTail: null | string;
};

/** Configured, and whether it is getting through.
 *
 *  The panel used to paint a green dot whenever `configured` was true.
 *  It was green for the two days the certificate on the mail host was
 *  expired and 110 notifications in a row failed. `healthy` is the
 *  last delivery's outcome, not a count: a week that delivered 52 and
 *  then failed 110 reads as fine by any measure that counts. */
export type SmtpStatus = {
  configured: boolean;
  host: string | null;
  from: string | null;
  delivered7d: number;
  failed7d: number;
  lastStatus: 'delivered' | 'failed' | null;
  lastError: string | null;
  lastDeliveredAt: string | null;
  healthy: boolean;
};

export type NotificationPref = {
  projectId: string;
  projectName: string;
  onNewIssue: boolean;
  onRegression: boolean;
};

const DEFAULT_BASE = '';

// ── instruments ──
export type AssertRow = {
  name: string;
  release: string;
  passCount: number;
  failCount: number;
  lastPassAt: string | null;
  lastFailAt: string | null;
};
export type ProbeRow = {
  ref: string;
  issueId: string | null;
  lastSeenRelease: string | null;
  registeredAt: string;
  lastFiredAt: string | null;
  fireCount: number;
};
export type TraceRow = {
  name: string;
  eventCount: number;
  usersCount: number;
  lastSeen: string;
  /** When we received it. `lastSeen` is the device's own clock. */
  lastReceivedAt?: string | null;
};
export type LaunchRow = {
  release: string;
  samples: number;
  prewarmed: number;
  p50: null | number;
  p90: null | number;
  p95: null | number;
};
export type InstrumentsPanel = {
  windowHours: number;
  users: number;
  activity: {at:string;events:number;errors:number;warnings:number;assertions:number;probes:number;traces:number}[];
  asserts: AssertRow[];
  probes: ProbeRow[];
  traces: TraceRow[];
  launch?: LaunchRow[];
};

class ApiError extends Error {
  /** What is wrong, when the server named it. The console owns the
   *  sentence; this is what it looks the sentence up by. */
  code?: string;
  /** Which field the code is about, when it is about one. */
  field?: string;
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

class Api {
  base = DEFAULT_BASE;
  private changes = new Set<(path: string) => void>();
  onChange(listener: (path: string) => void): () => void {
    this.changes.add(listener);
    return () => { this.changes.delete(listener); };
  }

  private async send<T>(method: string, path: string, body?: unknown, redirectOnUnauthorized = true, signal?: AbortSignal): Promise<T> {
    const resp = await fetch(`${this.base}${path}`, {
      method,
      signal,
      credentials: 'include',
      headers: body !== undefined && !(body instanceof FormData) ? { 'content-type': 'application/json' } : undefined,
      body: body instanceof FormData ? body : body !== undefined ? JSON.stringify(body) : undefined,
    });
    if (resp.status === 401 && redirectOnUnauthorized) {
      this.handleAuthFailure(path);
    }
    if (!resp.ok) {
      let detail = '';
      let code: string | undefined;
      let field: string | undefined;
      try {
        // A rejected credential answers with a code and the field it
        // is about. It used to answer with English prose, which the
        // console printed under a Chinese label — half a sentence in
        // each language, in the one place someone has already got
        // something wrong. The words are ours now; these are for
        // looking them up.
        const j = (await resp.json()) as {
          code?: string;
          detail?: string;
          error?: string;
          field?: string;
        };
        code = j.code;
        field = j.field;
        detail = [j.error, j.detail].filter(Boolean).join(' — ');
      } catch {
        // non-JSON error body
      }
      const err = new ApiError(resp.status, detail || `${resp.status}`);
      err.code = code;
      err.field = field;
      throw err;
    }
    const result = (await resp.json()) as T;
    if (method !== 'GET' && path.startsWith('/admin/api/')) {
      for (const listener of this.changes) listener(path);
    }
    return result;
  }

  private get<T>(path: string, signal?: AbortSignal): Promise<T> {
    return this.send<T>('GET', path, undefined, true, signal);
  }
  private post<T>(path: string, body?: unknown): Promise<T> {
    return this.send<T>('POST', path, body ?? {});
  }

  /** On 401 anywhere: stash the return path and go to /login. */
  private handleAuthFailure(path: string): void {
    const here = window.location.pathname;
    if (here === '/login' || path === '/auth/login') return;
    sessionStorage.setItem('sentori_return_to', here + window.location.search);
    window.location.href = '/login';
  }

  transfers() { return this.get<{ items: ProjectTransfer[]; configured: boolean; uploadLimit: number }>('/admin/api/transfers'); }
  createTransfer(command: TransferCreate) { return this.post('/admin/api/transfers', command); }
  transferCommand(id: string, command: TransferCommand) { return this.post(`/admin/api/transfers/${encodeURIComponent(id)}/commands`, command); }
  transferTail(id: string, command: TransferTail) { return this.post(`/admin/api/transfers/${encodeURIComponent(id)}/tail`, command); }
  uploadTransfer(id: string, upload: TransferUpload) {
    const form = new FormData();
    for (const [name, value] of Object.entries(upload.command)) form.set(name, String(value));
    form.set('archive', upload.file);
    return this.post(`/admin/api/transfers/${encodeURIComponent(id)}/upload?phase=${upload.phase}&version=${upload.version}`, form);
  }
  transferDownloadUrl(id: string, job: string) { return `${this.base}/admin/api/transfers/${encodeURIComponent(id)}/jobs/${encodeURIComponent(job)}/download`; }
  cancelTransferJob(id: string, job: string) { return this.post(`/admin/api/transfers/${encodeURIComponent(id)}/jobs/${encodeURIComponent(job)}/cancel`); }

  // Instance operations use authenticated server state, never VIS fixtures.
  systemInfo() { return this.get<SystemInfo>('/admin/api/system'); }
  backupDownloadUrl(id: string) { return `${this.base}/admin/api/instance/operations/${encodeURIComponent(id)}/download`; }
  verifyBackup(command: BackupCommand, file: File) {
    const form = new FormData();
    for (const [name, value] of Object.entries(command)) form.set(name, String(value));
    form.set('archive', file);
    return this.post<{ operation: InstanceOperation }>('/admin/api/instance/operations/verify', form);
  }
  instanceOperations() { return this.get<{ items: InstanceOperation[]; configured: boolean }>('/admin/api/instance/operations'); }
  prepareReset(command: ResetPreparation) { return this.post<ResetPermit>('/admin/api/instance/reset/prepare', command); }
  createBackup(command: BackupCommand) { return this.post<{ operation: InstanceOperation }>('/admin/api/instance/operations', command); }
  cancelBackup(id: string) { return this.post<{ operation: InstanceOperation }>(`/admin/api/instance/operations/${encodeURIComponent(id)}/cancel`); }
  instanceInfo() { return this.get<InstanceInfo>('/admin/api/instance'); }
  updateInstance(name: string, expectedVersion: number) {
    return this.send<InstanceInfo>('PATCH', '/admin/api/instance', { name, expectedVersion });
  }
  checkInstance() { return this.post<InstanceCheck>('/admin/api/instance/check'); }
  instanceHealth() { return this.get<InstanceHealth>('/admin/api/instance/health'); }
  initializeInstance(expectedVersion: number) {
    return this.post<InstanceInfo>('/admin/api/instance/initialize', { expectedVersion });
  }

  // ── auth ──
  async authConfig() {
    const value = await this.get<{ mode: 'password' | 'cloud'; controlOrigin?: string }>('/auth/config');
    if (value.mode !== 'password' && value.mode !== 'cloud') throw new ApiError(502, 'invalid_auth_configuration');
    if (value.mode === 'cloud') {
      const origin = new URL(value.controlOrigin ?? '');
      if (!['https:', 'http:'].includes(origin.protocol) || origin.origin !== value.controlOrigin) throw new ApiError(502, 'invalid_cloud_origin');
    }
    return value;
  }
  cloudUsage() { return this.get<CloudUsage>('/admin/api/cloud/usage'); }

  login(email: string, password: string) {
    return this.send<{ userId: string; role: Role }>('POST', '/auth/login', {
      email,
      password,
    });
  }
  logout() {
    return this.post<{ ok: boolean }>('/auth/logout');
  }
  authMe() {
    return this.get<Me>('/auth/me');
  }
  /** The public home checks for a session without sending visitors
   * to a login form. Network and server errors remain real errors. */
  async currentUser(): Promise<Me | null> {
    try {
      return await this.send<Me>('GET', '/auth/me', undefined, false);
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) return null;
      throw error;
    }
  }
  async changePassword(currentPassword: string, newPassword: string) {
    try {
      return await this.send<{ ok: boolean }>('POST', '/auth/change-password', {
        currentPassword,
        newPassword,
      }, false);
    } catch (error) {
      // A wrong current password is also 401. Keep the form open when the
      // session is still valid; only an expired session needs a fresh login.
      if (error instanceof ApiError && error.status === 401 && await this.currentUser() === null) {
        this.handleAuthFailure('/auth/change-password');
      }
      throw error;
    }
  }
  forgotPassword(email: string) {
    return this.send<{ ok: boolean }>('POST', '/auth/forgot-password', { email });
  }
  resetPassword(token: string, newPassword: string) {
    return this.send<{ ok: boolean }>('POST', '/auth/reset-password', {
      token,
      newPassword,
    });
  }

  // ── projects ──
  listProjects() {
    return this.get<{ projects: Project[] }>('/admin/api/projects');
  }
  getProject(id: string) {
    return this.get<{
      id: string;
      name: string;
      platform: string | null;
      version: number;
      createdAt: string;
      /** Only ever populated for a superadmin; the server returns null
       *  to everyone else, because whoever holds it can post into the
       *  team's channel. */
      webhookUrl: null | string;
    }>(`/admin/api/projects/${id}`);
  }
  createProject(name: string, platform?: string) {
    return this.post<{ id: string }>('/admin/api/projects', { name, platform });
  }
  /** `webhookUrl: ''` clears it. Absent leaves it alone — a PATCH
   *  that could not distinguish the two could never turn the channel
   *  off. */
  updateProject(
    id: string,
    patch: { name?: string; platform?: string; webhookUrl?: string; expectedVersion?: number },
  ) {
    return this.send<{ ok: boolean }>('PATCH', `/admin/api/projects/${id}`, patch);
  }
  createApp(projectId: string, app: {name: string; identifier: string; platform: string; kind: string}) {
    return this.send<{id: string}>('POST', `/admin/api/projects/${projectId}/apps`, app);
  }
  listApps(projectId: string) {
    return this.get<{apps: ProjectApp[]}>(`/admin/api/projects/${projectId}/apps`);
  }
  updateApp(projectId: string, appId: string, patch: {name?: string; repositoryUrl?: string; kind?: string; expectedVersion: number}) {
    return this.send<{ok: boolean; version: number}>('PATCH', `/admin/api/projects/${projectId}/apps/${appId}`, patch);
  }
  deleteProject(id: string, expectedVersion?: number) {
    const query = expectedVersion === undefined ? '' : `?expectedVersion=${expectedVersion}`;
    return this.send<{ ok: boolean }>('DELETE', `/admin/api/projects/${id}${query}`);
  }

  /** Deletes the release and, by cascade, its symbolication
   *  artifacts — after which that release's stacks cannot be
   *  symbolicated again until they are re-uploaded. */
  deleteRelease(releaseId: string) {
    return this.send<{ ok: boolean }>(
      'DELETE',
      `/admin/api/releases/${releaseId}`,
    );
  }

  // ── tokens ──
  listTokens(projectId: string) {
    return this.get<{ tokens: TokenRow[] }>(`/admin/api/projects/${projectId}/tokens`);
  }
  createToken(projectId: string, name: string, scope: 'api' | 'ingest') {
    return this.post<{ id: string; token: string }>(
      `/admin/api/projects/${projectId}/tokens`,
      { name, scope },
    );
  }
  revokeToken(tokenId: string) {
    return this.send<{ ok: boolean }>('DELETE', `/admin/api/tokens/${tokenId}`);
  }

  // ── users + assignments (owner) ──
  listUsers() {
    return this.get<{ users: UserRow[] }>('/admin/api/users');
  }
  createUser(email: string, password: string, displayName?: string) {
    return this.post<{ id: string }>('/admin/api/users', {
      email,
      password,
      displayName,
    });
  }
  deleteUser(userId: string) {
    return this.send<{ ok: boolean }>('DELETE', `/admin/api/users/${userId}`);
  }
  assignProject(userId: string, projectId: string) {
    return this.send<{ ok: boolean }>(
      'PUT',
      `/admin/api/users/${userId}/projects/${projectId}`,
    );
  }
  unassignProject(userId: string, projectId: string) {
    return this.send<{ ok: boolean }>(
      'DELETE',
      `/admin/api/users/${userId}/projects/${projectId}`,
    );
  }

  /** What the SDK's own traffic says about a project's deployment. */
  projectHealth(id: string) {
    return this.get<ProjectHealth>(`/admin/api/projects/${id}/health`);
  }

  taskIntegration(project: string) {
    return this.get<{ vaultConfigured: boolean; configuration: { repository: string; environment: string; enabled: boolean; version: number; secretConfigured: boolean } | null }>(`/admin/api/projects/${project}/task-integration`);
  }
  configureTaskIntegration(project: string, body: { repository: string; environment: string; enabled: boolean; expectedVersion: number; rotateSecret: boolean }) {
    return this.send<{ version: number; secret: string | null }>('PUT', `/admin/api/projects/${project}/task-integration`, body);
  }
  listTasks(project: string, query: URLSearchParams) {
    return this.get<{ tasks: Task[]; members: { id: string; name: string }[]; total: number; summary: {active:number;review:number;done:number;overdue:number}; nextOffset: number | null }>(`/admin/api/projects/${project}/tasks?${query}`);
  }
  listMonitors(project: string) { return this.get<{ monitors: Monitor[] }>(`/admin/api/projects/${project}/monitors`); }
  listIntegrations(project: string) { return this.get<{ integrations: Integration[]; vaultConfigured: boolean; smtpConfigured: boolean }>(`/admin/api/projects/${project}/integrations`); }
  configureIntegration(project: string, kind: IntegrationKind, body: IntegrationConfigure) { return this.send<{ id: string; version: number }>('PUT', `/admin/api/projects/${project}/integrations/${kind}`, body); }
  sendIntegration(project: string, id: string, body: IntegrationSend) { return this.post<{ deliveryId: string }>(`/admin/api/projects/${project}/integrations/${id}/send`, body); }
  listDeliveries(project: string, offset = 0) { return this.get<{ deliveries: Delivery[]; nextOffset: number | null }>(`/admin/api/projects/${project}/deliveries?offset=${offset}`); }
  retryDelivery(project: string, id: string, body: IntegrationRetry) { return this.post<{ deliveryId: string }>(`/admin/api/projects/${project}/deliveries/${id}/retry`, body); }
  monitorCommand(project: string, command: MonitorCommand) { return this.post<{ id: string; version: number; deleted: boolean }>(`/admin/api/projects/${project}/monitors/commands`, command); }
  monitorHistory(project: string, id: string, offset = 0) { return this.get<{ events: MonitorEvent[]; nextOffset: number | null }>(`/admin/api/projects/${project}/monitors/${id}/history?offset=${offset}`); }
  getTask(project: string, id: string, offset = 0) {
    return this.get<{ task: Task; events: TaskEvent[]; nextOffset: number | null }>(`/admin/api/projects/${project}/tasks/${id}?offset=${offset}`);
  }
  taskCommand(project: string, command: TaskCommand) {
    return this.post<Task>(`/admin/api/projects/${project}/tasks/commands`, command);
  }
  listInbox(query: URLSearchParams) {
    return this.get<{ items: InboxItem[]; total: number; unread: number; nextOffset: number | null }>(`/admin/api/inbox?${query}`);
  }
  readAllInbox() { return this.post<{ updated: number }>('/admin/api/inbox/read-all', {}); }
  updateInbox(id: string, change: { read?: boolean; archived?: boolean }) {
    return this.send<{ ok: boolean }>('PATCH', `/admin/api/inbox/${id}`, change);
  }
  inboxPreferences() { return this.get<InboxPreferences>('/admin/api/inbox-preferences'); }
  saveInboxPreferences(prefs: InboxPreferences) { return this.send<{ ok: boolean }>('PUT', '/admin/api/inbox-preferences', prefs); }

  // ── issues ──
  /** Crash-free rate for a window, and the same broken out per
   *  release — the comparison a ship/no-ship decision is made on. */
  crashFree(q: { appId?: string; projectId: string; hours?: number; environment?: string }) {
    const usp = new URLSearchParams({ projectId: q.projectId });
    if (q.hours) usp.set('hours', String(q.hours));
    if (q.appId) usp.set('appId', q.appId);
    if (q.environment) usp.set('environment', q.environment);
    return this.get<{
      crashFreeSessions: null | number;
      crashedSessions: number;
      releases: {
        crashFreeSessions: null | number;
        crashFreeUsers: null | number;
        crashedSessions: number;
        lastAt: string;
        platform: string;
        release: string;
        sessions: number;
        users: number;
      }[];
      crashFreeUsers: null | number;
      crashedUsers: number;
      sessions: number;
      /** Oldest first, one entry per bucket including the empty ones —
       *  a null rate is a stretch with no sessions, which is not the
       *  same as a healthy one. */
      trend: {
        at: string;
        crashFreeSessions: null | number;
        crashedSessions: number;
        sessions: number;
      }[];
      users: number;
      windowHours: number;
    }>(`/admin/api/sessions/crash-free?${usp.toString()}`);
  }

  listIssues(q: EventFilters & {
    status?: string;
    kind?: string;
    projectId?: string;
    limit?: number;
    offset?: number;
    q?: string;
    sort?: string;
    group?: string;
    environment?: string;
    contextKey?: string;
    contextValue?: string;
    release?: string;
  }, signal?: AbortSignal) {
    const usp = eventFilterParams(q);
    if (q.status) usp.set('status', q.status);
    if (q.kind) usp.set('kind', q.kind);
    if (q.projectId) usp.set('project_id', q.projectId);
    if (q.limit !== undefined) usp.set('limit', String(q.limit));
    if (q.offset !== undefined) usp.set('offset', String(q.offset));
    if (q.q) usp.set('q', q.q);
    if (q.sort) usp.set('sort', q.sort);
    if (q.group) usp.set('group', q.group);
    if (q.environment) usp.set('environment', q.environment);
    if (q.contextKey && q.contextValue) {
      usp.set('context_key', q.contextKey);
      usp.set('context_value', q.contextValue);
    }
    if (q.release) usp.set('release', q.release);
    const qs = usp.toString();
    return this.get<{ issues: IssueSummary[]; total: number; limit: number; offset: number; nextOffset: number | null; summary: { events: number; open: number; regressed: number; releases: number } }>(`/admin/api/issues${qs ? `?${qs}` : ''}`, signal);
  }
  projectEnvironments(projectId: string, filters: EventFilters = {}) {
    return this.get<{ environments: string[] }>(`/admin/api/projects/${projectId}/environments?${eventFilterParams(filters)}`);
  }
  projectContextKeys(projectId: string, filters: EventFilters = {}, source = 'context') {
    const params = eventFilterParams(filters); params.set('source', source);
    return this.get<{ keys: string[] }>(`/admin/api/projects/${projectId}/context-keys?${params}`);
  }
  projectContextValues(projectId: string, key: string, filters: EventFilters = {}, source = 'context') {
    const params = eventFilterParams(filters); params.set('source', source); params.set('key', key);
    return this.get<{ values: string[] }>(`/admin/api/projects/${projectId}/context-values?${params}`);
  }
  getIssue(id: string, signal?: AbortSignal) {
    return this.get<IssueDetail>(`/admin/api/issues/${id}`, signal);
  }
  resolveIssue(id: string, release?: string, note?: string) {
    return this.post<{ ok: boolean }>(`/admin/api/issues/${id}/resolve`, {
      release,
      note,
    });
  }
  ignoreIssue(id: string) {
    return this.post<{ ok: boolean }>(`/admin/api/issues/${id}/ignore`);
  }
  reopenIssue(id: string) {
    return this.post<{ ok: boolean }>(`/admin/api/issues/${id}/reopen`);
  }
  assignIssue(id: string, userId: string | null) {
    return this.post<{ ok: boolean }>(`/admin/api/issues/${id}/assign`, { userId });
  }
  addNote(id: string, body: string) {
    return this.post<{ ok: boolean }>(`/admin/api/issues/${id}/notes`, { body });
  }
  listOccurrences(id: string, offset = 0, limit = 100, filters: EventFilters = {}) {
    return this.get<{ events: OccurrenceRow[]; total: number; nextOffset: number | null }>(`/admin/api/issues/${id}/events?offset=${offset}&limit=${limit}&${eventFilterParams(filters)}`);
  }

  // ── events + attachments ──
  getEvent(id: string) {
    return this.get<EventDetail>(`/admin/api/events/${id}`);
  }
  attachmentUrl(ref: string): string {
    return `${this.base}/admin/api/attachments/${ref}`;
  }

  /** The other events this user's app reported in the minute
   *  around this one — the case timeline's third lane. */
  eventContext(id: string) {
    return this.get<{ events: ContextEventRow[] }>(`/admin/api/events/${id}/context`);
  }

  listJobs(projectId: string, releaseId: string) {
    return this.get<{ jobs: JobRow[] }>(`/admin/api/projects/${projectId}/jobs?releaseId=${releaseId}&limit=10`);
  }
  resymbolicateRelease(projectId: string, releaseId: string, idempotencyKey: string) {
    return this.post<{jobId: string}>(`/admin/api/projects/${projectId}/releases/${releaseId}/resymbolicate`, {idempotencyKey});
  }
  cancelJob(projectId: string, jobId: string) {
    return this.post<JobRow>(`/admin/api/projects/${projectId}/jobs/${jobId}/cancel`, {});
  }

  // ── releases ──
  listReleases(projectId: string, query: {id?:string;appId?:string;q?:string;names?:string[];offset?:number;limit?:number} = {}, signal?: AbortSignal) {
    const params = new URLSearchParams();
    if(query.id)params.set('id',query.id);
    if(query.q)params.set('q',query.q);
    if(query.names)params.set('names',JSON.stringify(query.names));
    if(query.appId)params.set('appId',query.appId);
    if(query.offset!==undefined)params.set('offset',String(query.offset));
    if(query.limit!==undefined)params.set('limit',String(query.limit));
    return this.get<{ releases: ReleaseRow[]; total: number; nextOffset: number | null }>(`/admin/api/projects/${projectId}/releases?${params}`, signal);
  }
  uploadArtifact(projectId: string, releaseId: string, kind: string, file: File) {
    const form = new FormData(); form.set('kind', kind); form.set('file', file);
    return this.post<{ id: string; usable: boolean; jobId: string }>(`/admin/api/projects/${projectId}/releases/${releaseId}/artifacts`, form);
  }
  listArtifacts(projectId: string, releaseId: string) {
    return this.get<{ artifacts: ArtifactRow[] }>(
      `/admin/api/projects/${projectId}/releases/${releaseId}/artifacts`,
    );
  }
  /** The instruments panel. It used to call `fetch` by hand and throw
   *  `new Error(String(resp.status))`, so its failure carried a bare
   *  number: no body read, no `ApiError`, and `formatApiError` — which
   *  exists to name the status and the message — had nothing to name.
   *  The banner said `Error: 500` while the server had answered with
   *  `upstream_unavailable — the database refused the connection`. */
  instruments(projectId: string, query: {q?:string;release?:string;hours?:number} = {}) {
    const params=new URLSearchParams();
    if(query.q)params.set('q',query.q);
    if(query.release)params.set('release',query.release);
    if(query.hours)params.set('hours',String(query.hours));
    return this.get<InstrumentsPanel>(`/admin/api/projects/${projectId}/instruments?${params}`);
  }

  // ── push ──
  /** What is configured, what is missing, and what that costs.
   *
   *  Codes and numbers, never sentences: the words live here, where
   *  they are translated and where the gate that keeps them
   *  translated runs. */
  pushReadiness(projectId: string) {
    return this.get<{ checks: PushCheck[]; live: number; ready: boolean }>(
      `/admin/api/projects/${projectId}/push/readiness`,
    );
  }
  pushHealth(projectId: string) {
    return this.get<PushHealth>(`/admin/api/projects/${projectId}/push/health`);
  }
  pushCredentials(projectId: string) {
    return this.get<{ credentials: PushCredential[]; vaultConfigured: boolean }>(
      `/admin/api/projects/${projectId}/push/credentials`,
    );
  }
  /** Adds one. Never replaces one — see migration 0017. `active`
   *  in the reply says whether it took over (nothing else of its
   *  kind was configured) or is staged awaiting promotion. */
  addPushCredential(
    projectId: string,
    provider: string,
    config: Record<string, unknown>,
    secret?: string,
    label?: string,
  ) {
    return this.post<{ id: string; active: boolean }>(
      `/admin/api/projects/${projectId}/push/credentials`,
      { provider, config, secret, label },
    );
  }
  /** Asks Apple or Google. Delivers nothing. */
  probePushCredential(projectId: string, credentialId: string) {
    return this.post<ProbeVerdict>(
      `/admin/api/projects/${projectId}/push/credentials/${credentialId}/probe`,
      {},
    );
  }
  /** Makes it the one that sends. Refused for a `rejected` or
   *  `limited` verdict unless forced. */
  activatePushCredential(projectId: string, credentialId: string, force = false) {
    return this.post<{ error?: string; ok?: boolean }>(
      `/admin/api/projects/${projectId}/push/credentials/${credentialId}/activate`,
      { force },
    );
  }
  deletePushCredential(projectId: string, credentialId: string) {
    return this.send<{ ok: boolean }>(
      'DELETE',
      `/admin/api/projects/${projectId}/push/credentials/${credentialId}`,
    );
  }
  /** Needs a device to aim at — a test send with no registered
   *  device is not a test, so the UI asks for one first. */
  pushTest(projectId: string, deviceTokenId: string, title: string, body: string) {
    return this.post<{ sendId?: string; error?: string }>(
      `/admin/api/projects/${projectId}/push/test`,
      { deviceTokenId, title, body },
    );
  }
  /** `scope` is the server's own parameter — `live` hides revoked
   *  rows, `all` shows them. It existed unused: a device that stopped
   *  receiving is exactly what someone opens this page to find, and
   *  the default view is the one view that cannot show it. */
  pushDevices(projectId: string, limit = 50, scope: 'all' | 'live' = 'live', offset = 0, search = '', provider = '') {
    return this.get<{ devices: PushDevice[]; offset: number; total: number }>(
      `/admin/api/projects/${projectId}/push/devices?limit=${limit}&scope=${scope}&offset=${offset}&q=${encodeURIComponent(search)}&provider=${encodeURIComponent(provider)}`,
    );
  }
  /** Retire a device from the console. The SDK can revoke its own
   *  registration and quarantine retires a dead token, but an
   *  operator looking at a device that should stop receiving had
   *  nowhere to click. */
  revokePushDevice(projectId: string, tokenId: string) {
    return this.post<{ status: string }>(
      `/admin/api/projects/${projectId}/push/devices/${tokenId}/revoke`,
      {},
    );
  }
  /** How many devices an audience selects, and a few of them.
   *
   *  The same compiled query the send runs, with a count in front, so
   *  the number is what a send would do rather than an estimate of
   *  it. */
  previewAudience(projectId: string, audience: AudienceRequest) {
    return this.post<{ matched: number; sample: AudienceSample[]; previewId: string; expiresAt: string }>(
      `/admin/api/projects/${projectId}/push/audience/preview`,
      audience,
    );
  }
  /** Send to everyone an audience selects.
   *
   *  `expectedMatched` is what the preview said. The server refuses
   *  with a 409 when it no longer holds — devices register between
   *  reading a number and pressing a button, and a notification
   *  cannot be recalled.
   *
   *  `idempotencyKey` is minted when the count is taken, so pressing
   *  send twice queues once. The count guard does not catch that on
   *  its own: sending does not change the audience, so the second
   *  press finds the same number and passes. */
  sendToAudience(
    projectId: string,
    audience: AudienceRequest,
    title: string,
    body: string,
    expectedMatched: number,
    idempotencyKey: string,
    previewId?: string,
  ) {
    return this.post<{ alreadySent: boolean; matched: number; queued: number }>(
      `/admin/api/projects/${projectId}/push/audience/send`,
      { ...audience, body, expectedMatched, idempotencyKey, title, previewId },
    );
  }
  pushSends(projectId: string, limit = 50, status = '', offset = 0, search = '', provider = '') {
    // The status filter goes to the server. Filtering a page in the
    // browser filters the page, not the data — with fifty of four
    // hundred rows in hand, "failed" showed the failures among the
    // fifty and called it the answer.
    const q = new URLSearchParams({ limit: String(limit), offset: String(offset) });
    if (status) q.set('status', status);
    if (search) q.set('q', search);
    if (provider) q.set('provider', provider);
    return this.get<{ offset: number; sends: PushSend[]; total: number }>(
      `/admin/api/projects/${projectId}/push/sends?${q.toString()}`,
    );
  }
  pushCommand(projectId: string, command: PushCommand) {
    return this.post<PushCommandResult>(`/admin/api/projects/${projectId}/push/commands`, command);
  }
  pushSendDetail(projectId: string, id: string) {
    return this.get<PushDetail>(`/admin/api/projects/${projectId}/push/sends/${id}`);
  }
  retryPushSend(projectId: string, sendId: string) {
    return this.post<{ status: string }>(
      `/admin/api/projects/${projectId}/push/sends/${sendId}/retry`,
    );
  }
  retryAllFailedPushSends(projectId: string) {
    return this.post<{ requeued: number }>(
      `/admin/api/projects/${projectId}/push/sends/_retry_all_failed`,
    );
  }

  // ── audit (owner) ──
  listAudit(limit = 100) {
    return this.get<{ entries: AuditRow[] }>(`/admin/api/audit?limit=${limit}`);
  }

  // ── attachments (blob fetch) ──
  async fetchAttachmentText(ref: string): Promise<string> {
    const resp = await fetch(`${this.base}/admin/api/attachments/${ref}`, {
      credentials: 'include',
    });
    if (!resp.ok) throw new Error(`attachment ${resp.status}`);
    return resp.text();
  }

  // ── notifications (email channel) ──
  smtpStatus() {
    return this.get<SmtpStatus>('/admin/api/smtp');
  }

  smtpTest() {
    return this.post<{ ok: boolean; to: string }>('/admin/api/smtp/test');
  }

  listNotificationPrefs() {
    return this.get<{ prefs: NotificationPref[] }>('/admin/api/notification-prefs');
  }

  putNotificationPref(pref: {
    projectId: string;
    onNewIssue: boolean;
    onRegression: boolean;
  }) {
    return this.send<{ ok: boolean }>('PUT', '/admin/api/notification-prefs', pref);
  }
}

export const api = new Api();
export { ApiError };
