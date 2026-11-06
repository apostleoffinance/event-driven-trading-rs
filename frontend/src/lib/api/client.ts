import { getAccessToken, refreshSession, clearSession } from "@/lib/auth/session";

export class ApiError extends Error {
  constructor(
    message: string,
    public readonly status: number,
    public readonly code?: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

function apiBaseUrl(): string {
  const value = process.env.NEXT_PUBLIC_API_URL?.trim();
  if (!value) throw new Error("NEXT_PUBLIC_API_URL is not configured. API mode cannot connect.");
  const normalized = value.replace(/\/$/, "");
  if (typeof window !== "undefined" && window.location.protocol === "https:" && normalized.startsWith("http://")) {
    throw new Error("API URL must use HTTPS when the frontend is served over HTTPS.");
  }
  return normalized;
}

type ApiRequestOptions = RequestInit & { auth?: boolean; retryAuth?: boolean };

export async function apiRequest<T>(path: string, options: ApiRequestOptions = {}): Promise<T> {
  const { auth = true, retryAuth = true, headers: inputHeaders, ...init } = options;
  const headers = new Headers(inputHeaders);
  if (init.body && !headers.has("Content-Type")) headers.set("Content-Type", "application/json");
  headers.set("Accept", "application/json");
  if (auth) {
    const token = getAccessToken();
    if (token) headers.set("Authorization", `Bearer ${token}`);
  }

  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl()}${path.startsWith("/") ? path : `/${path}`}`, {
      ...init, headers, cache: "no-store",
    });
  } catch {
    throw new ApiError("Backend unavailable or network/CORS request failed.", 0, "network_error");
  }

  if (response.status === 401 && auth && retryAuth && await refreshSession()) {
    const token = getAccessToken();
    if (token) headers.set("Authorization", `Bearer ${token}`);
    response = await fetch(`${apiBaseUrl()}${path.startsWith("/") ? path : `/${path}`}`, {
      ...init, headers, cache: "no-store",
    });
  }
  if (response.status === 401 && auth) clearSession();
  if (!response.ok) {
    let message = `API request failed (${response.status})`;
    let code: string | undefined;
    try {
      const body = await response.json() as { error?: { message?: string; code?: string } };
      message = body.error?.message ?? message;
      code = body.error?.code;
    } catch { /* response may not be JSON */ }
    throw new ApiError(message, response.status, code);
  }
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}

export function requireApiMode(): void {
  if (process.env.NEXT_PUBLIC_DATA_MODE !== "api") {
    throw new Error("This feature requires NEXT_PUBLIC_DATA_MODE=api.");
  }
  apiBaseUrl();
}
