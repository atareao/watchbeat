let token: string | null = null;

export function setToken(t: string | null) {
  token = t;
}

export function getToken(): string | null {
  if (token) return token;
  // Try cookie
  const match = document.cookie.match(/(?:^|;\s*)token=([^;]*)/);
  token = match?.[1] ?? null;
  return token;
}

export function logout() {
  token = null;
  document.cookie = 'token=; Path=/; Max-Age=0';
  window.location.href = '/login';
}