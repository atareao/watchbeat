export function getToken(): string | null {
  try {
    return sessionStorage.getItem('watchbeat_token') || localStorage.getItem('watchbeat_token');
  } catch {
    return null;
  }
}

export function setToken(token: string): void {
  try {
    sessionStorage.setItem('watchbeat_token', token);
    localStorage.setItem('watchbeat_token', token);
  } catch { /* noop */ }
}

export function clearToken(): void {
  try {
    sessionStorage.removeItem('watchbeat_token');
    localStorage.removeItem('watchbeat_token');
    sessionStorage.removeItem('watchbeat_user');
  } catch { /* noop */ }
}