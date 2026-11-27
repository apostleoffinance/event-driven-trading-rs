import { clearSession, refreshSession } from "@/lib/auth/session";

export class ApiError extends Error {
  constructor(message: string, public readonly status: number, public readonly code?: string) {
    super(message); this.name = "ApiError";
  }
}

/** All browser API traffic is same-origin and passes through the HttpOnly-cookie BFF. */
export function apiBaseUrl(service?: "identity" | "journal" | "market-data"): string {
  return `/api/bff/${service ?? "gateway"}`;
}

const ACTIVE_ORGANIZATION_KEY = "edgenest.organization.active.v1";
const ACTIVE_WORKSPACE_KEY = "edgenest.workspace.active.v1";

async function resolveOrganizationId(): Promise<string> {
  if (typeof window === "undefined") throw new ApiError("Choose an organization before loading trading data.", 409, "organization_required");
  const stored = window.sessionStorage.getItem(ACTIVE_ORGANIZATION_KEY);
  if (stored) return stored;
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl()}/api/v1/organizations`, {
      headers: { Accept: "application/json" }, credentials: "same-origin", cache: "no-store",
    });
  } catch {
    throw new ApiError("We couldn't load your organizations. Check your connection and try again.", 0, "network_error");
  }
  if (!response.ok) throw new ApiError("We couldn't verify your organization access. Please refresh and try again.", response.status, "organization_lookup_failed");
  const body = await response.json() as { data?: Array<{ id: string; status: string }> };
  const available = (body.data ?? []).filter((org) => org.status === "ACTIVE");
  if (available.length === 1) {
    window.sessionStorage.setItem(ACTIVE_ORGANIZATION_KEY, available[0].id);
    return available[0].id;
  }
  if (available.length === 0) throw new ApiError("Your account isn't part of an active organization yet. Ask your organization owner to invite you or create an organization.", 409, "organization_required");
  throw new ApiError("You belong to more than one organization. Choose which organization you want to work in.", 409, "organization_selection_required");
}

async function resolveWorkspaceId(organizationId: string): Promise<string> {
  if (typeof window === "undefined") throw new ApiError("Choose a workspace before loading trading data.", 409, "workspace_required");
  const stored = window.sessionStorage.getItem(ACTIVE_WORKSPACE_KEY);
  if (stored) return stored;
  let response: Response;
  try {
    response = await fetch(`${apiBaseUrl()}/api/v1/organizations/${encodeURIComponent(organizationId)}/workspaces`, {
      headers: { Accept: "application/json", "X-Organization-Id": organizationId },
      credentials: "same-origin", cache: "no-store",
    });
  } catch {
    throw new ApiError("We couldn't load your workspaces. Check your connection and try again.", 0, "network_error");
  }
  if (!response.ok) throw new ApiError("We couldn't verify your workspace access. Refresh and try again.", response.status, "workspace_lookup_failed");
  const body = await response.json() as { data?: Array<{ id: string; status: string }> };
  const available = (body.data ?? []).filter((workspace) => workspace.status === "ACTIVE");
  if (available.length === 1) {
    window.sessionStorage.setItem(ACTIVE_WORKSPACE_KEY, available[0].id);
    return available[0].id;
  }
  if (available.length === 0) throw new ApiError("You don't have access to an active workspace. Ask an organization owner or administrator for access.", 403, "workspace_access_required");
  throw new ApiError("Choose a workspace before loading trading data.", 409, "workspace_selection_required");
}

type ApiRequestOptions = RequestInit & { auth?: boolean; retryAuth?: boolean; baseUrl?: string };

export async function apiRequest<T>(path: string, options: ApiRequestOptions = {}): Promise<T> {
  const { auth = true, retryAuth = true, headers: inputHeaders, baseUrl, ...init } = options;
  const headers = new Headers(inputHeaders);
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
  if (init.body && !(init.body instanceof FormData) && !headers.has("Content-Type")) headers.set("Content-Type", "application/json");
  headers.set("Accept", "application/json");
  const url = `${(baseUrl || apiBaseUrl()).replace(/\/$/, "")}${path.startsWith("/") ? path : `/${path}`}`;
  let response: Response;
  try {
    response = await fetch(url, { ...init, headers, credentials: "same-origin", cache: "no-store" });
  } catch {
    throw new ApiError("Unable to connect to EdgeNest services. Please try again.", 0, "network_error");
  }
  if (response.status === 401 && auth && retryAuth && await refreshSession()) {
    try {
      response = await fetch(url, { ...init, headers, credentials: "same-origin", cache: "no-store" });
    } catch {
      throw new ApiError("Unable to connect to EdgeNest services. Please try again.", 0, "network_error");
    }
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
  const responseBody = await response.text();
  if (!responseBody.trim()) return undefined as T;
  try { return JSON.parse(responseBody) as T; }
  catch { throw new ApiError("The service returned an invalid response. Please try again.", response.status, "invalid_response"); }
}

export function requireApiMode(): void {
  const configured = process.env.NEXT_PUBLIC_API_URL || process.env.NEXT_PUBLIC_IDENTITY_API_URL || process.env.NEXT_PUBLIC_JOURNAL_API_URL || process.env.NEXT_PUBLIC_MARKET_DATA_API_URL;
  if (!configured?.trim()) throw new Error("This service is temporarily unavailable. Please try again later.");
}
