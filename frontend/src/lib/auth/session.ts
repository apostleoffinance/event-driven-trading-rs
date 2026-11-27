"use client";

import { apiRequest } from "@/lib/api/client";

export type SessionUser = { id: string; email: string; display_name: string };
const SESSION_USER_KEY = "edgenest.auth.user.v2";
const REFRESH_COMPLETED_KEY = "edgenest.auth.refresh.completedAt";

function readUser(): SessionUser | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = window.localStorage.getItem(SESSION_USER_KEY);
    return raw ? JSON.parse(raw) as SessionUser : null;
  } catch { return null; }
}
function writeUser(user: SessionUser): void {
  window.localStorage.setItem(SESSION_USER_KEY, JSON.stringify(user));
}
export function getSessionUser(): SessionUser | null { return readUser(); }
export function hasSession(): boolean { return Boolean(readUser()); }
export function clearSession(): void {
  if (typeof window !== "undefined") window.localStorage.removeItem(SESSION_USER_KEY);
}

let csrfTokenPromise: Promise<string> | null = null;
async function csrfToken(): Promise<string> {
  if (typeof document !== "undefined") {
    const existing = document.cookie.split("; ").find((part) => part.startsWith("edgenest_csrf="))?.split("=").slice(1).join("=");
    if (existing && decodeURIComponent(existing).length >= 32) return decodeURIComponent(existing);
  }
  if (!csrfTokenPromise) {
    csrfTokenPromise = (async () => {
      const response = await fetch("/api/auth/csrf", { method: "GET", credentials: "same-origin", cache: "no-store" });
      if (!response.ok) throw new Error("Unable to establish a secure session. Refresh the page and try again.");
      const payload = await response.json() as { data?: { csrf_token?: string } };
      const token = payload.data?.csrf_token;
      if (!token) throw new Error("Unable to establish a secure session. Refresh the page and try again.");
      return token;
    })().finally(() => { csrfTokenPromise = null; });
  }
  return csrfTokenPromise;
}
async function csrfHeaders(): Promise<Headers> {
  return new Headers({ "Content-Type": "application/json", "X-CSRF-Token": await csrfToken() });
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
  window.localStorage.removeItem(REFRESH_COMPLETED_KEY);
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
async function rotateSessionOnce(): Promise<boolean> {
  const lastCompleted = Number(window.localStorage.getItem(REFRESH_COMPLETED_KEY) || "0");
  // Another tab may already have rotated the shared HttpOnly cookie while this tab
  // was waiting for the cross-tab Web Lock.
  if (lastCompleted > 0 && Date.now() - lastCompleted < 15_000) return true;
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
    window.localStorage.setItem(REFRESH_COMPLETED_KEY, String(Date.now()));
    return true;
  } catch {
    clearSession();
    return false;
  }
}
export async function refreshSession(): Promise<boolean> {
  if (typeof window === "undefined") return false;
  if (refreshInFlight) return refreshInFlight;
  refreshInFlight = (async () => {
    try {
      if (navigator.locks) {
        return await navigator.locks.request("edgenest-auth-refresh", rotateSessionOnce);
      }
      return await rotateSessionOnce();
    } catch {
      clearSession();
      return false;
    } finally { refreshInFlight = null; }
  })();
  return refreshInFlight;
}
