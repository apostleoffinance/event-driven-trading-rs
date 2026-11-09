import { getAccessToken, refreshSession, clearSession } from "@/lib/auth/session";

export class ApiError extends Error {
  constructor(message: string, public readonly status: number, public readonly code?: string) {
    super(message); this.name = "ApiError";
  }
}

export function apiBaseUrl(service?: "identity" | "journal" | "market-data"): string {
  const specific = service === "identity" ? process.env.NEXT_PUBLIC_IDENTITY_API_URL
    : service === "journal" ? process.env.NEXT_PUBLIC_JOURNAL_API_URL
    : service === "market-data" ? process.env.NEXT_PUBLIC_MARKET_DATA_API_URL : undefined;
  const value = (specific || process.env.NEXT_PUBLIC_API_URL || "").trim();
  if (!value) {
    throw new Error("This service is temporarily unavailable. Please try again later.");
  }
  const normalized = value.replace(/\/$/, "");
  if (typeof window !== "undefined" && window.location.protocol === "https:" && normalized.startsWith("http://")) {
    throw new Error("API URL must use HTTPS when the frontend is served over HTTPS.");
  }
  return normalized;
}

type ApiRequestOptions = RequestInit & { auth?: boolean; retryAuth?: boolean; baseUrl?: string };

export async function apiRequest<T>(path: string, options: ApiRequestOptions = {}): Promise<T> {
  const { auth = true, retryAuth = true, headers: inputHeaders, baseUrl, ...init } = options;
  const headers = new Headers(inputHeaders);
  if (init.body && !headers.has("Content-Type")) headers.set("Content-Type", "application/json");
  headers.set("Accept", "application/json");
  if (auth) {
    const token = getAccessToken();
    if (token) headers.set("Authorization", `Bearer ${token}`);
  }
  const url = `${(baseUrl || apiBaseUrl()).replace(/\/$/, "")}${path.startsWith("/") ? path : `/${path}`}`;
  let response: Response;
  try { response = await fetch(url, { ...init, headers, cache: "no-store" }); }
  catch { throw new ApiError("Unable to connect to EdgeNest services. Please try again.", 0, "network_error"); }

  if (response.status === 401 && auth && retryAuth && await refreshSession()) {
    const token = getAccessToken();
    if (token) headers.set("Authorization", `Bearer ${token}`);
    try { response = await fetch(url, { ...init, headers, cache: "no-store" }); }
    catch { throw new ApiError("Unable to connect to EdgeNest services. Please try again.", 0, "network_error"); }
  }
  if (response.status === 401 && auth) clearSession();
  if (!response.ok) {
    let message = `API request failed (${response.status})`;
    let code: string | undefined;
    try {
      const body = await response.json() as { error?: { message?: string; code?: string } };
      message = body.error?.message ?? message; code = body.error?.code;
    } catch { /* non-JSON error response */ }
    throw new ApiError(message, response.status, code);
  }
  if (response.status === 204) return undefined as T;
  return response.json() as Promise<T>;
}

export function requireApiMode(): void {
  const configured = process.env.NEXT_PUBLIC_API_URL || process.env.NEXT_PUBLIC_IDENTITY_API_URL || process.env.NEXT_PUBLIC_JOURNAL_API_URL || process.env.NEXT_PUBLIC_MARKET_DATA_API_URL;
  if (!configured?.trim()) throw new Error("This service is temporarily unavailable. Please try again later.");
}
