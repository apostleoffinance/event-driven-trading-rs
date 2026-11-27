import { clearSession } from "@/lib/auth/session";

export class ApiError extends Error {
  constructor(message: string, public readonly status: number, public readonly code?: string) {
    super(message);
    this.name = "ApiError";
  }
}

export function apiBaseUrl(service?: "identity" | "journal" | "market-data"): string {
  const specific = service === "identity" ? process.env.NEXT_PUBLIC_IDENTITY_API_URL
    : service === "journal" ? process.env.NEXT_PUBLIC_JOURNAL_API_URL
    : service === "market-data" ? process.env.NEXT_PUBLIC_MARKET_DATA_API_URL : undefined;
  const value = (specific || process.env.NEXT_PUBLIC_API_URL || "").trim();
  if (!value) throw new Error("This service is temporarily unavailable. Please try again later.");
  const normalized = value.replace(/\/$/, "");
  if (typeof window !== "undefined" && window.location.protocol === "https:" && normalized.startsWith("http://")) {
    throw new Error("API URL must use HTTPS when the frontend is served over HTTPS.");
  }
  return normalized;
}

const ACTIVE_ORGANIZATION_KEY = "edgenest.organization.active.v1";
const ACTIVE_WORKSPACE_KEY = "edgenest.workspace.active.v1";

function serviceFor(baseUrl?: string): "identity" | "journal" | "market-data" | "gateway" {
  if (!baseUrl) return "gateway";
  if (baseUrl === apiBaseUrl("identity")) return "identity";
  if (baseUrl === apiBaseUrl("journal")) return "journal";
  if (baseUrl === apiBaseUrl("market-data")) return "market-data";
  return "gateway";
}

function readCsrfCookie(): string | null {
  if (typeof document === "undefined") return null;
  for (const name of ["edgenest_csrf", "__Host-edgenest_csrf"]) {
    const prefix = `${name}=`;
    const value = document.cookie.split(";").map(part => part.trim()).find(part => part.startsWith(prefix));
    if (value) {
      try { return decodeURIComponent(value.slice(prefix.length)); } catch { return null; }
    }
  }
  return null;
}

async function csrfToken(): Promise<string> {
  let token = readCsrfCookie();
  if (token) return token;
  const response = await fetch("/api/bff/csrf", { method: "GET", credentials: "same-origin", cache: "no-store" });
  if (!response.ok) throw new ApiError("Unable to establish a secure session. Refresh the page and try again.", response.status, "csrf_bootstrap_failed");
  token = readCsrfCookie();
  if (!token) throw new ApiError("Unable to establish a secure session. Refresh the page and try again.", 503, "csrf_cookie_missing");
  return token;
}

async function resolveOrganizationId(): Promise<string> {
  if (typeof window === "undefined") throw new ApiError("Choose an organization before loading trading data.", 409, "organization_required");
  const stored = window.sessionStorage.getItem(ACTIVE_ORGANIZATION_KEY);
  if (stored) return stored;

  const response = await apiRequest<{ data?: Array<{ id: string; status: string }> }>("/api/v1/organizations");
  const available = (response.data ?? []).filter(org => org.status === "ACTIVE");
  if (available.length === 1) {
    window.sessionStorage.setItem(ACTIVE_ORGANIZATION_KEY, available[0].id);
    return available[0].id;
  }
  if (available.length === 0) {
    throw new ApiError("Your account isn't part of an active organization yet. Ask your organization owner to invite you or create an organization.", 409, "organization_required");
  }
  throw new ApiError("You belong to more than one organization. Choose which organization you want to work in.", 409, "organization_selection_required");
}

async function resolveWorkspaceId(organizationId: string): Promise<string> {
  if (typeof window === "undefined") throw new ApiError("Choose a workspace before loading trading data.", 409, "workspace_required");
  const stored = window.sessionStorage.getItem(ACTIVE_WORKSPACE_KEY);
  if (stored) return stored;

  const response = await apiRequest<{ data?: Array<{ id: string; status: string }> }>(
    `/api/v1/organizations/${encodeURIComponent(organizationId)}/workspaces`,
    { headers: { "X-Organization-Id": organizationId } },
  );
  const available = (response.data ?? []).filter(workspace => workspace.status === "ACTIVE");
  if (available.length === 1) {
    window.sessionStorage.setItem(ACTIVE_WORKSPACE_KEY, available[0].id);
    return available[0].id;
  }
  if (available.length === 0) throw new ApiError("You don't have access to an active workspace. Ask an organization owner or administrator for access.", 403, "workspace_access_required");
  throw new ApiError("Choose a workspace before loading trading data.", 409, "workspace_selection_required");
}

type ApiRequestOptions = RequestInit & { auth?: boolean; retryAuth?: boolean; baseUrl?: string };

export async function apiRequest<T>(path: string, options: ApiRequestOptions = {}): Promise<T> {
  const { auth = true, retryAuth: _retryAuth = true, headers: inputHeaders, baseUrl, ...init } = options;
  const headers = new Headers(inputHeaders);
  headers.delete("Authorization");
  const requiresOrganization =
    path.startsWith("/api/v1/trading/") ||
    (path.startsWith("/api/v1/organizations/") && path !== "/api/v1/organizations");
  if (auth && requiresOrganization && !headers.has("X-Organization-Id")) {
    headers.set("X-Organization-Id", await resolveOrganizationId());
  }
  if (auth && path.startsWith("/api/v1/trading/") && !headers.has("X-Workspace-Id")) {
    const organizationId = headers.get("X-Organization-Id") ?? await resolveOrganizationId();
    headers.set("X-Workspace-Id", await resolveWorkspaceId(organizationId));
  }
  if (init.body && !headers.has("Content-Type") && !(init.body instanceof FormData)) {
    headers.set("Content-Type", "application/json");
  }
  headers.set("Accept", "application/json");

  const method = (init.method ?? "GET").toUpperCase();
  if (!["GET", "HEAD", "OPTIONS"].includes(method)) headers.set("X-CSRF-Token", await csrfToken());
  const service = serviceFor(baseUrl);
  const url = `/api/bff/${service}${path.startsWith("/") ? path : `/${path}`}`;

  let response: Response;
  try {
    response = await fetch(url, {
      ...init,
      method,
      headers,
      credentials: "same-origin",
      cache: "no-store",
    });
  } catch {
    throw new ApiError("Unable to connect to EdgeNest services. Please try again.", 0, "network_error");
  }

  if (response.status === 401 && auth) clearSession();
  if (!response.ok) {
    let message = `API request failed (${response.status})`;
    let code: string | undefined;
    try {
      const body = await response.json() as { error?: { message?: string; code?: string } };
      message = body.error?.message ?? message;
      code = body.error?.code;
    } catch { /* non-JSON error response */ }
    throw new ApiError(message, response.status, code);
  }
  if (response.status === 204) return undefined as T;
  const responseBody = await response.text();
  if (!responseBody.trim()) return undefined as T;
  try { return JSON.parse(responseBody) as T; }
  catch { throw new ApiError("The service returned an invalid response. Please try again.", response.status, "invalid_response"); }
}

export function requireApiMode(): void {
  const configured = process.env.NEXT_PUBLIC_API_URL || process.env.NEXT_PUBLIC_IDENTITY_API_URL || process.env.NEXT_PUBLIC_JOURNAL_API_URL || process.env.NEXT_PUBLIC_MARKET_DATA_API_URL;
  if (!configured?.trim()) throw new Error("This service is temporarily unavailable. Please try again later.");
}
