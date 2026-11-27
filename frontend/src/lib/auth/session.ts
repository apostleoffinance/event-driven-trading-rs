"use client";

import { ApiError, apiRequest } from "@/lib/api/client";

export type SessionUser = { id: string; email: string; display_name: string };
const SESSION_KEY = "edgenest.auth.session.v2";
let sessionVerified = false;

function readUser(): SessionUser | null {
  if (typeof window === "undefined") return null;
  try {
    const raw = window.sessionStorage.getItem(SESSION_KEY);
    if (!raw) return null;
    const value = JSON.parse(raw) as { user?: SessionUser };
    return value.user?.id && value.user.email ? value.user : null;
  } catch { return null; }
}

function writeUser(user: SessionUser): void {
  if (typeof window !== "undefined") {
    // This is a non-authoritative UI cache only. Session credentials never enter Web Storage.
    const safeUser: SessionUser = { id: user.id, email: user.email, display_name: user.display_name || user.email };
    window.sessionStorage.setItem(SESSION_KEY, JSON.stringify({ user: safeUser }));
    window.sessionStorage.removeItem("edgenest.auth.session.v1");
    sessionVerified = true;
  }
}

export function getSessionUser(): SessionUser | null { return readUser(); }
export function hasSession(): boolean { return Boolean(readUser()); }
export function clearSession(): void {
  if (typeof window !== "undefined") {
    window.sessionStorage.removeItem(SESSION_KEY);
    window.sessionStorage.removeItem("edgenest.auth.session.v1");
    sessionVerified = false;
  }
}

/** Kept temporarily for callers migrating to the BFF; browser access tokens are no longer exposed. */
export function getAccessToken(): string | null { return null; }

export async function login(email: string, password: string): Promise<SessionUser> {
  const response = await apiRequest<{ user: SessionUser }>("/api/v1/auth/login", {
    method: "POST", auth: false, service: "identity",
    body: JSON.stringify({ email, password }),
  });
  if (!response.user?.id) throw new Error("The identity service returned an invalid session.");
  writeUser(response.user);
  return response.user;
}

export async function register(email: string, display_name: string, password: string): Promise<SessionUser> {
  const response = await apiRequest<{ user: SessionUser }>("/api/v1/auth/register", {
    method: "POST", auth: false, service: "identity",
    body: JSON.stringify({ email, display_name, password }),
  });
  if (!response.user?.id) throw new Error("The identity service returned an invalid session.");
  writeUser(response.user);
  return response.user;
}

let restoreInFlight: Promise<SessionUser | null> | null = null;
type BrowserLockManager = { request<T>(name: string, callback: () => Promise<T>): Promise<T> };

/** Restores the non-sensitive user hint by asking the same-origin BFF to validate the HttpOnly session. */
export async function restoreSession(): Promise<SessionUser | null> {
  if (typeof window !== "undefined") window.sessionStorage.removeItem("edgenest.auth.session.v1");
  const cached = readUser();
  if (sessionVerified && cached) return cached;
  if (restoreInFlight) return restoreInFlight;
  restoreInFlight = (async () => {
    try {
      const verifyUnderBrowserLock = async (): Promise<SessionUser | null> => {
        const payload = await apiRequest<{ authenticated: boolean; user?: SessionUser }>("", {
          method: "POST",
          auth: false,
          service: "session",
          body: JSON.stringify({}),
        });
        if (!payload.authenticated) {
          clearSession();
          return null;
        }
        if (!payload.user?.id || !payload.user.email) throw new Error("Invalid identity response.");
        writeUser(payload.user);
        return payload.user;
      };
      const locks = typeof navigator !== "undefined"
        ? (navigator as Navigator & { locks?: BrowserLockManager }).locks
        : undefined;
      // The Web Lock serializes refresh across tabs. The BFF also coalesces
      // duplicate refreshes within a Worker isolate as a second safety layer.
      return locks
        ? await locks.request("edgenest-bff-session-refresh", verifyUnderBrowserLock)
        : await verifyUnderBrowserLock();
    } catch (error) {
      if (error instanceof ApiError && error.status === 401) {
        clearSession();
        return null;
      }
      // Keep a previously cached display hint through transient API outages;
      // the backend still authorizes every operation using the HttpOnly cookie.
      return cached;
    } finally {
      restoreInFlight = null;
    }
  })();
  return restoreInFlight;
}

export async function refreshSession(): Promise<boolean> {
  sessionVerified = false;
  return Boolean(await restoreSession());
}

export async function logout(): Promise<void> {
  try {
    await apiRequest<void>("/api/v1/auth/logout", {
      method: "POST", auth: false, service: "identity",
      body: JSON.stringify({}),
    });
  } finally {
    clearSession();
  }
}

export async function getCurrentUser(): Promise<SessionUser> {
  const user = await restoreSession();
  if (!user) throw new ApiError("Sign in to continue.", 401, "unauthorized");
  return user;
}
