"use client";

import { apiRequest } from "@/lib/api/client";

export type SessionUser = { id: string; email: string; display_name: string };
const SESSION_USER_KEY = "edgenest.auth.user.v2";

function readUser(): SessionUser | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = window.sessionStorage.getItem(SESSION_USER_KEY);
    return raw ? JSON.parse(raw) as SessionUser : null;
  } catch { return null; }
}
function writeUser(user: SessionUser): void {
  window.sessionStorage.setItem(SESSION_USER_KEY, JSON.stringify(user));
}
export function getSessionUser(): SessionUser | null { return readUser(); }
export function hasSession(): boolean { return Boolean(readUser()); }
export function clearSession(): void {
  if (typeof window !== "undefined") window.sessionStorage.removeItem(SESSION_USER_KEY);
}

async function csrfHeaders(): Promise<Headers> {
  const response = await fetch("/api/auth/csrf", { method: "GET", credentials: "same-origin", cache: "no-store" });
  if (!response.ok) throw new Error("Unable to establish a secure session. Refresh the page and try again.");
  const payload = await response.json() as { data?: { csrf_token?: string } };
  const token = payload.data?.csrf_token;
  if (!token) throw new Error("Unable to establish a secure session. Refresh the page and try again.");
  return new Headers({ "Content-Type": "application/json", "X-CSRF-Token": token });
}

async function authMutation(path: string, body?: unknown): Promise<SessionUser> {
  const headers = await csrfHeaders();
  const response = await fetch(path, {
    method: "POST",
    headers,
    credentials: "same-origin",
    cache: "no-store",
    body: JSON.stringify(body ?? {}),
  });
  const payload = await response.json().catch(() => ({})) as {
    user?: SessionUser;
    error?: { message?: string };
  };
  if (!response.ok || !payload.user) throw new Error(payload.error?.message || "Authentication failed.");
  writeUser(payload.user);
  return payload.user;
}

export async function login(email: string, password: string): Promise<SessionUser> {
  return authMutation("/api/bff/identity/api/v1/auth/login", { email, password });
}
export async function register(email: string, display_name: string, password: string): Promise<SessionUser> {
  return authMutation("/api/bff/identity/api/v1/auth/register", { email, display_name, password });
}

let refreshInFlight: Promise<boolean> | null = null;
export async function refreshSession(): Promise<boolean> {
  if (typeof window === "undefined") return false;
  if (refreshInFlight) return refreshInFlight;
  refreshInFlight = (async () => {
    try {
      const headers = await csrfHeaders();
      const response = await fetch("/api/bff/identity/api/v1/auth/refresh", {
        method: "POST", headers, credentials: "same-origin", cache: "no-store", body: "{}",
      });
      const payload = await response.json().catch(() => ({})) as { user?: SessionUser };
      if (!response.ok || !payload.user) {
        clearSession();
        return false;
      }
      writeUser(payload.user);
      return true;
    } catch {
      clearSession();
      return false;
    } finally { refreshInFlight = null; }
  })();
  return refreshInFlight;
}

export async function logout(): Promise<void> {
  try {
    const headers = await csrfHeaders();
    const response = await fetch("/api/bff/identity/api/v1/auth/logout", {
      method: "POST", headers, credentials: "same-origin", cache: "no-store", body: "{}",
    });
    if (!response.ok && response.status !== 401) throw new Error("Unable to complete sign out.");
  } finally { clearSession(); }
}

export async function getCurrentUser(): Promise<SessionUser> {
  return apiRequest<{ data: SessionUser }>("/api/v1/auth/me", { baseUrl: "/api/bff/identity" }).then((r) => r.data);
}
