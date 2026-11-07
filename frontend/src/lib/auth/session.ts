"use client";

import { apiBaseUrl, apiRequest } from "@/lib/api/client";
const identityBaseUrl = () => apiBaseUrl("identity");

type AuthPayload = {
  token_type: "Bearer";
  access_token: string;
  expires_in: number;
  refresh_token: string;
  user: { id: string; email: string; display_name: string };
};
export type SessionUser = AuthPayload["user"];

const SESSION_KEY = "edgenest.auth.session.v1";
type StoredSession = AuthPayload & { expires_at: number };

function readSession(): StoredSession | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = sessionStorage.getItem(SESSION_KEY);
    if (!raw) return null;
    const value = JSON.parse(raw) as StoredSession;
    if (!value.access_token || !value.refresh_token || !value.expires_at) return null;
    return value;
  } catch { return null; }
}
function writeSession(payload: AuthPayload): void {
  sessionStorage.setItem(SESSION_KEY, JSON.stringify({ ...payload, expires_at: Date.now() + payload.expires_in * 1000 }));
}
export function getAccessToken(): string | null {
  const session = readSession();
  if (!session) return null;
  return session.expires_at > Date.now() + 30_000 ? session.access_token : null;
}
export function getSessionUser(): SessionUser | null { return readSession()?.user ?? null; }
export function hasSession(): boolean { return Boolean(readSession()?.refresh_token); }
export function clearSession(): void {
  if (typeof window !== "undefined") sessionStorage.removeItem(SESSION_KEY);
}
export async function login(email: string, password: string): Promise<SessionUser> {
  const payload = await apiRequest<AuthPayload>("/api/v1/auth/login", {
    method: "POST", auth: false, retryAuth: false, baseUrl: identityBaseUrl(), body: JSON.stringify({ email, password }),
  });
  writeSession(payload);
  return payload.user;
}
export async function register(email: string, display_name: string, password: string): Promise<SessionUser> {
  const payload = await apiRequest<AuthPayload>("/api/v1/auth/register", {
    method: "POST", auth: false, retryAuth: false, baseUrl: identityBaseUrl(), body: JSON.stringify({ email, display_name, password }),
  });
  writeSession(payload);
  return payload.user;
}
let refreshInFlight: Promise<boolean> | null = null;
export async function refreshSession(): Promise<boolean> {
  if (typeof window === "undefined") return false;
  if (refreshInFlight) return refreshInFlight;
  const session = readSession();
  if (!session?.refresh_token) return false;
  refreshInFlight = (async () => {
    try {
      const payload = await apiRequest<AuthPayload>("/api/v1/auth/refresh", {
        method: "POST", auth: false, retryAuth: false, baseUrl: identityBaseUrl(),
        body: JSON.stringify({ refresh_token: session.refresh_token }),
      });
      writeSession(payload);
      return true;
    } catch {
      clearSession();
      return false;
    } finally { refreshInFlight = null; }
  })();
  return refreshInFlight;
}
export async function logout(): Promise<void> {
  const session = readSession();
  try {
    if (session?.refresh_token) {
      await apiRequest<void>("/api/v1/auth/logout", {
        method: "POST", auth: true, retryAuth: false, baseUrl: identityBaseUrl(),
        body: JSON.stringify({ refresh_token: session.refresh_token }),
      });
    }
  } finally { clearSession(); }
}
export async function getCurrentUser(): Promise<SessionUser> {
  return apiRequest<{ data: SessionUser }>("/api/v1/auth/me", { baseUrl: identityBaseUrl() }).then((r) => r.data);
}
