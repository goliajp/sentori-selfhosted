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
  platform: string;
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

export type ProjectHealth = {
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
  id: string;
  name: string;
  created_at?: string;
  createdAt?: string;
};

export type ArtifactRow = {
  id: string;
  kind: string;
  name: string;
  content_hash?: string;
  size_bytes?: number;
  created_at?: string;
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

export type SmtpStatus =
  | { configured: true; host: string; from: string }
  | { configured: false };

export type NotificationPref = {
  projectId: string;
  projectName: string;
  onNewIssue: boolean;
  onRegression: boolean;
};

const DEFAULT_BASE = '';

class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

class Api {
  base = DEFAULT_BASE;

  private async send<T>(method: string, path: string, body?: unknown): Promise<T> {
    const resp = await fetch(`${this.base}${path}`, {
      method,
      credentials: 'include',
      headers: body !== undefined ? { 'content-type': 'application/json' } : undefined,
      body: body !== undefined ? JSON.stringify(body) : undefined,
    });
    if (resp.status === 401) {
      this.handleAuthFailure(path);
    }
    if (!resp.ok) {
      let detail = '';
      try {
        const j = (await resp.json()) as { error?: string };
        detail = j.error ?? '';
      } catch {
        // non-JSON error body
      }
      throw new ApiError(resp.status, detail || `${resp.status}`);
    }
    return (await resp.json()) as T;
  }

  private get<T>(path: string): Promise<T> {
    return this.send<T>('GET', path);
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

  // ── auth ──
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
  changePassword(currentPassword: string, newPassword: string) {
    return this.post<{ ok: boolean }>('/auth/change-password', {
      currentPassword,
      newPassword,
    });
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
  createProject(name: string, platform?: string) {
    return this.post<{ id: string }>('/admin/api/projects', { name, platform });
  }
  updateProject(id: string, patch: { name?: string; platform?: string }) {
    return this.send<{ ok: boolean }>('PATCH', `/admin/api/projects/${id}`, patch);
  }
  deleteProject(id: string) {
    return this.send<{ ok: boolean }>('DELETE', `/admin/api/projects/${id}`);
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

  // ── issues ──
  listIssues(q: {
    status?: string;
    kind?: string;
    projectId?: string;
    limit?: number;
    environment?: string;
    contextKey?: string;
    contextValue?: string;
    release?: string;
  }) {
    const usp = new URLSearchParams();
    if (q.status) usp.set('status', q.status);
    if (q.kind) usp.set('kind', q.kind);
    if (q.projectId) usp.set('project_id', q.projectId);
    if (q.limit) usp.set('limit', String(q.limit));
    if (q.environment) usp.set('environment', q.environment);
    if (q.contextKey && q.contextValue) {
      usp.set('context_key', q.contextKey);
      usp.set('context_value', q.contextValue);
    }
    if (q.release) usp.set('release', q.release);
    const qs = usp.toString();
    return this.get<{ issues: IssueSummary[] }>(`/admin/api/issues${qs ? `?${qs}` : ''}`);
  }
  /** Deployment environments this project's events have reported. */
  projectEnvironments(projectId: string) {
    return this.get<{ environments: string[] }>(
      `/admin/api/projects/${projectId}/environments`,
    );
  }
  /** Context keys this project's events have reported — slicing
   *  dimensions whose meaning belongs to the host, not to Sentori. */
  projectContextKeys(projectId: string) {
    return this.get<{ keys: string[] }>(
      `/admin/api/projects/${projectId}/context-keys`,
    );
  }
  projectContextValues(projectId: string, key: string) {
    return this.get<{ values: string[] }>(
      `/admin/api/projects/${projectId}/context-values?key=${encodeURIComponent(key)}`,
    );
  }
  getIssue(id: string) {
    return this.get<IssueDetail>(`/admin/api/issues/${id}`);
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
  listOccurrences(id: string) {
    return this.get<{ events: OccurrenceRow[] }>(`/admin/api/issues/${id}/events`);
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

  // ── releases ──
  listReleases(projectId: string) {
    return this.get<{ releases: ReleaseRow[] }>(
      `/admin/api/projects/${projectId}/releases`,
    );
  }
  listArtifacts(projectId: string, releaseId: string) {
    return this.get<{ artifacts: ArtifactRow[] }>(
      `/admin/api/projects/${projectId}/releases/${releaseId}/artifacts`,
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
