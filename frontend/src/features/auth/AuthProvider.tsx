"use client";

import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { usePathname, useRouter } from "next/navigation";
import { frontendConfig } from "@/config/frontend";

export type IdentityUser = { id: string; email: string; display_name: string };
type TokenResponse = { access_token: string; expires_in: number; user: IdentityUser };
type AuthContextValue = {
  user: IdentityUser | null;
  accessToken: string | null;
  ready: boolean;
  login: (email: string, password: string) => Promise<{ mfaRequired: boolean; challengeToken?: string }>;
  completeMfaLogin: (challengeToken: string, code: string) => Promise<void>;
  register: (email: string, displayName: string, password: string) => Promise<void>;
  verifyEmail: (token: string) => Promise<void>;
  resendVerification: (email: string) => Promise<void>;
  requestPasswordReset: (email: string) => Promise<void>;
  confirmPasswordReset: (token: string, password: string) => Promise<void>;
  logout: () => Promise<void>;
  authFetch: (url: string, init?: RequestInit) => Promise<Response>;
  identityFetch: (url: string, init?: RequestInit) => Promise<Response>;
};
const AuthContext = createContext<AuthContextValue | null>(null);
const identityBase = frontendConfig.identityUrl.replace(/\/$/, "");
const journalBase = frontendConfig.journalApiUrl.replace(/\/$/, "");

async function parseResponse<T>(response: Response): Promise<T> {
  const body = await response.json().catch(() => ({}));
  if (!response.ok) {
    const message = body?.error?.message ?? body?.message ?? `Request failed (${response.status})`;
    throw new Error(message);
  }
  return body as T;
}
async function identityRequest<T>(path: string, init: RequestInit = {}) {
  if (!identityBase) throw new Error("Identity API URL is not configured.");
  return parseResponse<T>(await fetch(`${identityBase}${path}`, {
    ...init, credentials: "include", headers: { "Content-Type": "application/json", ...(init.headers ?? {}) },
  }));
}

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [user, setUser] = useState<IdentityUser | null>(null);
  const [accessToken, setAccessToken] = useState<string | null>(null);
  const [ready, setReady] = useState(false);
  const refreshPromise = useRef<Promise<string | null> | null>(null);

  const accept = useCallback((tokens: TokenResponse) => {
    setAccessToken(tokens.access_token);
    setUser(tokens.user);
  }, []);

  useEffect(() => {
    let alive = true;
    identityRequest<TokenResponse>("/api/v1/auth/refresh", { method: "POST", body: "{}" })
      .then((tokens) => { if (alive) { setAccessToken(tokens.access_token); setUser(tokens.user); } })
      .catch(() => { if (alive) { setAccessToken(null); setUser(null); } })
      .finally(() => { if (alive) setReady(true); });
    return () => { alive = false; };
  }, []);

  const login = useCallback(async (email: string, password: string) => {
    const result = await identityRequest<TokenResponse & { mfa_required?: boolean; challenge_token?: string }>(
      "/api/v1/auth/login", { method: "POST", body: JSON.stringify({ email, password }) });
    if (result.mfa_required && result.challenge_token) return { mfaRequired: true, challengeToken: result.challenge_token };
    accept(result);
    return { mfaRequired: false };
  }, [accept]);

  const completeMfaLogin = useCallback(async (challengeToken: string, code: string) => {
    const result = await identityRequest<TokenResponse>("/api/v1/auth/mfa/verify-login", {
      method: "POST", body: JSON.stringify({ challenge_token: challengeToken, code }),
    });
    accept(result);
  }, [accept]);

  const register = useCallback(async (email: string, displayName: string, password: string) => {
    await identityRequest("/api/v1/auth/register", { method: "POST", body: JSON.stringify({ email, display_name: displayName, password }) });
  }, []);
  const verifyEmail = useCallback(async (token: string) => {
    await identityRequest("/api/v1/auth/verify-email", { method: "POST", body: JSON.stringify({ token }) });
  }, []);
  const resendVerification = useCallback(async (email: string) => {
    await identityRequest("/api/v1/auth/resend-verification", { method: "POST", body: JSON.stringify({ email }) });
  }, []);
  const requestPasswordReset = useCallback(async (email: string) => {
    await identityRequest("/api/v1/auth/password-reset/request", { method: "POST", body: JSON.stringify({ email }) });
  }, []);
  const confirmPasswordReset = useCallback(async (token: string, password: string) => {
    await identityRequest("/api/v1/auth/password-reset/confirm", { method: "POST", body: JSON.stringify({ token, new_password: password }) });
  }, []);

  const logout = useCallback(async () => {
    try {
      if (accessToken) await identityRequest("/api/v1/auth/logout", { method: "POST", headers: { Authorization: `Bearer ${accessToken}` }, body: "{}" });
    } finally {
      setAccessToken(null); setUser(null);
    }
  }, [accessToken]);

  const refreshAccessToken = useCallback(async (): Promise<string | null> => {
    if (refreshPromise.current) return refreshPromise.current;
    const pending = (async () => {
      try {
        const tokens = await identityRequest<TokenResponse>("/api/v1/auth/refresh", {
          method: "POST",
          body: "{}",
        });
        accept(tokens);
        return tokens.access_token;
      } catch {
        setAccessToken(null);
        setUser(null);
        return null;
      } finally {
        refreshPromise.current = null;
      }
    })();
    refreshPromise.current = pending;
    return pending;
  }, [accept]);

  const authFetch = useCallback(async (url: string, init: RequestInit = {}) => {
    if (!accessToken) throw new Error("Sign in to continue.");
    if (!journalBase && !url.startsWith("http")) throw new Error("Journal API URL is not configured.");
    const target = url.startsWith("http") ? url : `${journalBase}${url}`;
    const request = (token: string) => fetch(target, {
      ...init,
      credentials: "include",
      headers: { Authorization: `Bearer ${token}`, ...(init.headers ?? {}) },
    });
    let response = await request(accessToken);
    if (response.status === 401) {
      const renewed = await refreshAccessToken();
      if (!renewed) throw new Error("Your session expired. Sign in again.");
      response = await request(renewed);
      if (response.status === 401) {
        setAccessToken(null);
        setUser(null);
        throw new Error("Your session expired. Sign in again.");
      }
    }
    return response;
  }, [accessToken, refreshAccessToken]);

  const identityFetch = useCallback(async (url: string, init: RequestInit = {}) => {
    if (!accessToken) throw new Error("Sign in to continue.");
    const request = (token: string) => fetch(`${identityBase}${url}`, {
      ...init,
      credentials: "include",
      headers: { Authorization: `Bearer ${token}`, ...(init.headers ?? {}) },
    });
    let response = await request(accessToken);
    if (response.status === 401 && url !== "/api/v1/auth/refresh") {
      const renewed = await refreshAccessToken();
      if (!renewed) throw new Error("Your session expired. Sign in again.");
      response = await request(renewed);
      if (response.status === 401) {
        setAccessToken(null);
        setUser(null);
        throw new Error("Your session expired. Sign in again.");
      }
    }
    return response;
  }, [accessToken, refreshAccessToken]);

  const value = useMemo(() => ({ user, accessToken, ready, login, completeMfaLogin, register, verifyEmail, resendVerification, requestPasswordReset, confirmPasswordReset, logout, authFetch, identityFetch }), [user, accessToken, ready, login, completeMfaLogin, register, verifyEmail, resendVerification, requestPasswordReset, confirmPasswordReset, logout, authFetch, identityFetch]);
  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth() {
  const value = useContext(AuthContext);
  if (!value) throw new Error("useAuth must be used within AuthProvider");
  return value;
}

const publicRoutes = ["/login", "/register", "/verify-email", "/forgot-password", "/reset-password"];
export function AuthGate({ children }: { children: React.ReactNode }) {
  const { ready, user } = useAuth();
  const pathname = usePathname();
  const router = useRouter();
  const isPublic = publicRoutes.some((route) => pathname === route || pathname.startsWith(route + "/"));
  useEffect(() => {
    if (ready && !user && !isPublic) router.replace("/login");
  }, [ready, user, isPublic, router]);
  if (isPublic) return <>{children}</>;
  if (!ready || !user) return <div className="loading">Checking EdgeNest session…</div>;
  return <>{children}</>;
}
