import { ApiError, type CloudUsage } from './api';

export type CloudAccount = { id: string; email: string; displayName: string; emailVerified: boolean };
export type CloudOrganization = { id: string; name: string; version: number; role: 'owner' | 'member' };
export type CloudWorkspaceRow = { id: string; organizationId: string; name: string; region: string; status: 'draft'|'checkout'|'paid'|'provisioning'|'ready'|'suspended'|'closed'; version: number };
export type CloudPage<T> = { items: T[]; nextOffset: number | null };
export type CloudStatus = { registration: boolean; checkout: boolean; provisioning: boolean; billingMode: 'test'|'live'|'fixture'|null };
export type CloudCatalog = { revision: string; regions: { id: string; name: string }[]; offers: { id: string; name: string; currency: string; unitAmount: number; interval: 'month'|'year'; taxBehavior: 'exclusive'|'inclusive'; limits: CloudUsage['limits'] }[]; terms: { revision: string; serviceUrl: string; privacyUrl: string; refundUrl: string; cancellation: string } };
export type CloudQuote = {id:string; expiresAt:string; acceptUntil:string; expired?:boolean; snapshot:{catalogRevision:string; region:string; quantity:number; offer:CloudCatalog['offers'][number]; terms:CloudCatalog['terms']}};
export type CloudBilling = {checkouts:{id:string; quoteId:string; status:string; livemode:boolean; checkoutUrl:string|null; subscriptionStatus:string|null; paidUntil:string|null; cancelAtPeriodEnd:boolean; expiresAt:string; lastCheckedAt:string|null; error:string|null}[]; invoices:{id:string; currency:string; amountPaid:number; paidUntil:string}[]; commands:{id:string; status:string; cancelAtPeriodEnd:boolean; error:string|null}[]};
export type CloudAccess = {workspaceId: string; organizationId: string; name: string; origin: string};

export function cloudReturnTo(): string {
  const next = sessionStorage.getItem('sentori_cloud_return_to');
  sessionStorage.removeItem('sentori_cloud_return_to');
  return next?.startsWith('/cloud/') && !next.includes('\\') && !next.includes('#') && !next.startsWith('/cloud/login') ? next : '/cloud';
}

async function request<T>(path: string, body?: unknown, authenticated = true): Promise<T> {
  const response = await fetch('/cloud/api' + path, { method: body === undefined ? 'GET' : 'POST', credentials: 'same-origin', headers: body === undefined ? undefined : {'Content-Type':'application/json'}, body: body === undefined ? undefined : JSON.stringify(body) });
  if (!response.ok) {
    const value = await response.json().catch(() => ({})) as {error?:string};
    if (response.status === 401 && authenticated) {
      sessionStorage.setItem('sentori_cloud_return_to', location.pathname + location.search);
      location.assign('/cloud/login');
    }
    const error = new ApiError(response.status, value.error ?? String(response.status));
    error.code = value.error;
    throw error;
  }
  return response.json() as Promise<T>;
}

export const cloudApi = {
  status: () => request<CloudStatus>('/status', undefined, false),
  catalog: () => request<{configured:boolean;catalog:CloudCatalog|null}>('/catalog', undefined, false),
  me: () => request<CloudAccount>('/auth/me'),
  login: (email: string, password: string) => request('/auth/login', {email, password}, false),
  requestLink: (email: string, purpose: 'register'|'reset') => request('/auth/request-link', {email, purpose}, false),
  complete: (token: string, purpose: string, password: string, displayName: string) => request('/auth/complete', {token, purpose, password, displayName}, false),
  password: (currentPassword: string, newPassword: string) => request('/auth/password', {currentPassword, newPassword}, false),
  logout: () => request('/auth/logout', {}),
  organizations: (offset = 0) => request<CloudPage<CloudOrganization>>('/organizations?offset=' + offset),
  organization: (id: string) => request<{organization: Omit<CloudOrganization,'role'>; members: {id:string; email:string; displayName:string; role:'owner'|'member'}[]; invitations:{id:string; email:string; expiresAt:string}[]}>('/organizations/' + encodeURIComponent(id)),
  workspace: (org: string, id: string) => request<{workspace:CloudWorkspaceRow; serviceGrant:{kind:'internal'|'trial';expiresAt:string|null;active:boolean}|null; provisioning:{state:string;attempts:number;error:string|null;updatedAt:string}|null; members:{id:string; email:string; displayName:string; role:'admin'|'member'}[]}>('/organizations/' + encodeURIComponent(org) + '/workspaces/' + encodeURIComponent(id)),
  quotes: (org:string,id:string) => request<CloudPage<CloudQuote>>('/organizations/' + encodeURIComponent(org) + '/workspaces/' + encodeURIComponent(id) + '/quotes'),
  billing: (org:string,id:string) => request<CloudBilling>('/organizations/' + encodeURIComponent(org) + '/workspaces/' + encodeURIComponent(id) + '/billing'),
  workspaces: (id: string, offset = 0) => request<CloudPage<CloudWorkspaceRow>>('/organizations/' + encodeURIComponent(id) + '/workspaces?offset=' + offset),
  invitations: (offset = 0) => request<CloudPage<{id:string; organizationId:string; organizationName:string; expiresAt:string}>>('/invitations?offset=' + offset),
  acceptInvitation: (id: string) => request('/invitations/' + encodeURIComponent(id) + '/accept', {}),
  access: (workspace: string) => request<CloudAccess>('/workspaces/' + encodeURIComponent(workspace) + '/access'),
  async entry(workspace: string): Promise<string> {
    const result = await request<CloudAccess>('/workspaces/' + encodeURIComponent(workspace) + '/access');
    const url = new URL(result.origin);
    if (url.origin !== result.origin || (url.protocol !== 'https:' && !(url.protocol === 'http:' && ['localhost','127.0.0.1','[::1]'].includes(url.hostname)))) throw new ApiError(502,'invalid_cloud_origin');
    return url.origin + '/auth/cloud/start';
  },
  ticket: (org: string, workspace: string, challenge: string) => request<{url:string}>('/organizations/' + encodeURIComponent(org) + '/workspaces/' + encodeURIComponent(workspace) + '/access-ticket', {challenge}),
  /** Keep an uncertain command's exact key across retries and page reloads. */
  async command<T>(actor: string, path: string, body: Record<string,unknown>): Promise<T> {
    const slot = 'sentori_cloud_command:' + actor + ':' + path + ':' + JSON.stringify(body);
    let commandId = sessionStorage.getItem(slot);
    if (!commandId) { commandId = crypto.randomUUID(); sessionStorage.setItem(slot, commandId); }
    try {
      const result = await request<T>(path, {...body, commandId});
      sessionStorage.removeItem(slot);
      return result;
    } catch (error) {
      if (error instanceof ApiError && error.status >= 400 && error.status < 500) sessionStorage.removeItem(slot);
      throw error;
    }
  },
};
