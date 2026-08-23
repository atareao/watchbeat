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

import { getToken } from '../store/auth';

async function fetcher<T>(path: string, opts?: { method?: string; body?: unknown }): Promise<T> {
  const token = getToken();
  const headers: Record<string, string> = { 'Content-Type': 'application/json' };
  if (token) headers['Authorization'] = `Bearer ${token}`;

  const res = await fetch(path, {
    method: opts?.method ?? (opts?.body ? 'POST' : 'GET'),
    headers,
    body: opts?.body ? JSON.stringify(opts.body) : undefined,
  });

  if (!res.ok) {
    const text = await res.text().catch(() => 'unknown error');
    throw new Error(`HTTP ${res.status}: ${text}`);
  }

  if (res.status === 204) return undefined as T;
  return res.json();
}

export async function fetchMe(): Promise<User> {
  return fetcher<User>('/api/me');
}

export async function fetchMonitors(): Promise<{ monitors: Monitor[] }> {
  return fetcher('/api/monitors');
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

export async function fetchChecks(id: string, limit = 50, offset = 0): Promise<{ checks: CheckResult[] }> {
  return fetcher(`/api/monitors/${id}/checks?limit=${limit}&offset=${offset}`);
}

export async function fetchTimeline(id: string, days = 1): Promise<{ timeline: TimelinePoint[] }> {
  return fetcher(`/api/monitors/${id}/timeline?days=${days}`);
}

export async function fetchStatus(): Promise<{
  status: DashboardStatus;
  monitors: MonitorSummary[];
  scheduler: { last_run_at: string | null; next_run_at: string | null; last_monitors_checked: number };
}> {
  return fetcher('/api/status');
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