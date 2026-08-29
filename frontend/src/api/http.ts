import { getToken, clearToken } from "../store/auth";

export interface User {
  sub: string;
  email?: string;
  name?: string;
}

export interface Monitor {
  id: string;
  name: string;
  type: string;
  target: string;
  config_json: Record<string, unknown>;
  interval_seconds: number;
  timeout_seconds: number;
  enabled: boolean;
  notifier_id: string | null;
  latency_threshold_ms: number | null;
  message_template_down: string | null;
  message_template_latency: string | null;
  message_template_up: string | null;
  message_template_expiry: string | null;
  token: string | null;
  grace_seconds: number | null;
  last_seen_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface CheckResult {
  id: number;
  monitor_id: string;
  status: string;
  status_code: number | null;
  response_time_ms: number;
  error_message: string | null;
  checked_at: string;
  tls_cert_expires_at: string | null;
  tls_cert_days_left: number | null;
}

export interface MonitorSummary {
  id: string;
  name: string;
  monitor_type: string;
  target: string;
  enabled: boolean;
  last_status: string | null;
  last_response_time_ms: number | null;
  last_checked_at: string | null;
  uptime_7d: number | null;
  uptime_30d: number | null;
  token: string | null;
  grace_seconds: number | null;
  last_seen_at: string | null;
}

export interface DashboardStatus {
  total_monitors: number;
  enabled_monitors: number;
  up_monitors: number;
  down_monitors: number;
  total_checks_24h: number;
  avg_response_time_24h: number | null;
}

export interface Notifier {
  id: string;
  name: string;
  type: string;
  config_json: Record<string, string>;
  enabled: boolean;
  created_at: string;
  updated_at: string;
}

export interface TimelinePoint {
  checked_at: string;
  status: string;
  response_time_ms: number | null;
}

export interface TimelineBucket {
  bucket_start: string;
  up_pct: number;
  avg_response_time_ms: number;
  count: number;
  dominant_status: string;
}

async function fetcher<T>(path: string, opts?: { method?: string; body?: unknown }): Promise<T> {
  const token = getToken();
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;

  const res = await fetch(path, {
    method: opts?.method ?? (opts?.body ? 'POST' : 'GET'),
    headers,
    body: opts?.body ? JSON.stringify(opts.body) : undefined,
  });

  // If unauthorized, clear token and redirect to login
  if (res.status === 401) {
    clearToken();
    window.location.href = '/login';
    throw new Error('Not authenticated');
  }

  if (!res.ok) {
    const text = await res.text().catch(() => 'unknown error');
    throw new Error(`HTTP ${res.status}: ${text}`);
  }

  if (res.status === 204) return undefined as T;
  return res.json();
}

export async function fetchMe(): Promise<User> {
  const data = await fetcher<{ authenticated: boolean; user: User }>('/api/me');
  if (!data.authenticated || !data.user) throw new Error('Not authenticated');
  return data.user;
}

export interface PaginatedResponse<T> {
  monitors: T[];
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}

export interface UnifiedDashboardResponse {
  status: DashboardStatus;
  monitors: MonitorSummary[];
  scheduler: { last_run_at: string | null; next_run_at: string | null; last_monitors_checked: number };
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}

export async function fetchMonitors(params?: {
  page?: number;
  perPage?: number;
  q?: string;
  type?: string;
  status?: string;
}): Promise<UnifiedDashboardResponse> {
  const searchParams = new URLSearchParams();
  if (params?.page) searchParams.set('page', String(params.page));
  if (params?.perPage) searchParams.set('per_page', String(params.perPage));
  if (params?.q) searchParams.set('q', params.q);
  if (params?.type) searchParams.set('type', params.type);
  if (params?.status) searchParams.set('status', params.status);
  const qs = searchParams.toString();
  return fetcher(`/api/monitors${qs ? `?${qs}` : ''}`);
}

export async function fetchMonitor(id: string): Promise<Monitor> {
  return fetcher<Monitor>(`/api/monitors/${id}`);
}

export async function createMonitor(data: Partial<Monitor>): Promise<Monitor> {
  return fetcher<Monitor>('/api/monitors', { method: 'POST', body: data });
}

export async function updateMonitor(id: string, data: Partial<Monitor>): Promise<Monitor> {
  return fetcher<Monitor>(`/api/monitors/${id}`, { method: 'PUT', body: data });
}

export async function deleteMonitor(id: string): Promise<void> {
  return fetcher(`/api/monitors/${id}`, { method: 'DELETE' });
}

export async function toggleMonitor(id: string): Promise<{ enabled: boolean }> {
  return fetcher(`/api/monitors/${id}`, { method: 'PATCH' });
}

export async function runCheck(id: string): Promise<CheckResult> {
  return fetcher<CheckResult>(`/api/monitors/${id}/check`, { method: 'POST' });
}

export async function fetchChecks(id: string, page = 1, perPage = 20): Promise<{
  checks: CheckResult[];
  total: number;
  page: number;
  per_page: number;
  total_pages: number;
}> {
  const offset = (page - 1) * perPage;
  return fetcher(`/api/monitors/${id}/checks?limit=${perPage}&offset=${offset}`);
}

export async function fetchTimeline(id: string, opts?: { days?: number; hours?: number }): Promise<{ timeline: TimelinePoint[] }> {
  if (opts?.hours != null) {
    return fetcher(`/api/monitors/${id}/timeline?hours=${opts.hours}`);
  }
  const days = opts?.days ?? 1;
  return fetcher(`/api/monitors/${id}/timeline?days=${days}`);
}

export async function fetchTimelineBuckets(
  id: string,
  opts: { bucket_seconds: number } & ({ days?: number } | { hours?: number }),
): Promise<{ buckets: TimelineBucket[] }> {
  let query: string;
  if ('hours' in opts && opts.hours != null) {
    query = `hours=${opts.hours}&bucket_seconds=${opts.bucket_seconds}`;
  } else {
    const days = (opts as { days?: number }).days ?? 1;
    query = `days=${days}&bucket_seconds=${opts.bucket_seconds}`;
  }
  return fetcher(`/api/monitors/${id}/timeline?${query}`);
}

export async function fetchStatus(): Promise<UnifiedDashboardResponse> {
  return fetcher('/api/monitors?per_page=100');
}

export async function fetchNotifiers(): Promise<{ notifiers: Notifier[] }> {
  return fetcher('/api/notifiers');
}

export async function createNotifier(data: Partial<Notifier>): Promise<Notifier> {
  return fetcher<Notifier>('/api/notifiers', { method: 'POST', body: data });
}

export async function updateNotifier(id: string, data: Partial<Notifier>): Promise<Notifier> {
  return fetcher<Notifier>(`/api/notifiers/${id}`, { method: 'PUT', body: data });
}

export async function deleteNotifier(id: string): Promise<void> {
  return fetcher(`/api/notifiers/${id}`, { method: 'DELETE' });
}

export async function testNotifier(id: string): Promise<{ sent: boolean }> {
  return fetcher(`/api/notifiers/${id}/test`, { method: 'POST' });
}

// ───── Status Pages ─────

export interface StatusPage {
  id: string;
  slug: string;
  title: string;
  description: string | null;
  monitors: string[];
  public: boolean;
  created_at: string;
  updated_at: string;
}

export async function fetchStatusPages(): Promise<{ status_pages: StatusPage[] }> {
  return fetcher('/api/status-pages');
}

export async function createStatusPage(data: Partial<StatusPage>): Promise<StatusPage> {
  return fetcher<StatusPage>('/api/status-pages', { method: 'POST', body: data });
}

export async function updateStatusPage(id: string, data: Partial<StatusPage>): Promise<StatusPage> {
  return fetcher<StatusPage>(`/api/status-pages/${id}`, { method: 'PUT', body: data });
}

export async function deleteStatusPage(id: string): Promise<void> {
  return fetcher(`/api/status-pages/${id}`, { method: 'DELETE' });
}

// ───── Settings ─────

export async function fetchSetting(key: string): Promise<{ key: string; value: string | null }> {
  return fetcher(`/api/settings?key=${encodeURIComponent(key)}`);
}

export async function saveSetting(key: string, value: string): Promise<void> {
  await fetcher('/api/settings', {
    method: 'POST',
    body: { key, value },
  });
}

export async function createBackup(): Promise<{ path: string }> {
  return fetcher('/api/backup', { method: 'POST' });
}

// ───── Export / Import ─────

export interface ExportPayload {
  version: string;
  exported_at: string;
  monitors: Record<string, unknown>[];
  notifiers: Record<string, unknown>[];
  status_pages: Record<string, unknown>[];
  settings: { key: string; value: string }[];
}

export async function exportConfig(): Promise<ExportPayload> {
  return fetcher<ExportPayload>('/api/export');
}

export async function importConfig(payload: ExportPayload): Promise<{ ok: boolean; imported: { monitors: number; notifiers: number; status_pages: number; settings: number } }> {
  return fetcher('/api/import', { method: 'POST', body: payload });
}

// ───── Heartbeats ─────

export interface Heartbeat {
  id: string;
  name: string;
  token: string;
  grace_seconds: number;
  last_seen_at: string | null;
  status: string;
  notifier_id: string | null;
  created_at: string;
  updated_at: string;
}

export async function fetchHeartbeats(): Promise<{ heartbeats: Heartbeat[] }> {
  return fetcher('/api/heartbeats');
}

export async function createHeartbeat(data: Partial<Heartbeat>): Promise<Heartbeat> {
  return fetcher<Heartbeat>('/api/heartbeats', { method: 'POST', body: data });
}

export async function updateHeartbeat(id: string, data: Partial<Heartbeat>): Promise<Heartbeat> {
  return fetcher<Heartbeat>(`/api/heartbeats/${id}`, { method: 'PUT', body: data });
}

export async function deleteHeartbeat(id: string): Promise<void> {
  return fetcher(`/api/heartbeats/${id}`, { method: 'DELETE' });
}

// ───── Unified Dashboard Item ─────

export type DashboardItem = MonitorSummary;
