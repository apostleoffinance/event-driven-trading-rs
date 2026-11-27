import { getCloudflareContext } from "@opennextjs/cloudflare";

type Service = "identity" | "journal" | "market-data" | "gateway";
type AuthTokens = {
  access_token: string;
  refresh_token: string;
  expires_in: number;
  user: { id: string; email: string; display_name: string };
};
type RuntimeEnv = Record<string, string | undefined>;

const ACCESS_COOKIE = "edgenest_access";
const REFRESH_COOKIE = "edgenest_refresh";
const CSRF_COOKIE = "edgenest_csrf";
const SAFE_METHODS = new Set(["GET", "HEAD", "OPTIONS"]);
const PUBLIC_MARKET_DATA_PATHS = [
  "/api/v1/market-data/providers",
  "/api/v1/market-data/instruments",
  "/api/v1/market-data/candles",
  "/api/v1/market-data/fx/convert",
  "/api/v1/market-data/mt5/latest",
];

function cookies(request: Request): Map<string, string> {
  const result = new Map<string, string>();
  for (const part of (request.headers.get("cookie") ?? "").split(";")) {
    const separator = part.indexOf("=");
    if (separator < 1) continue;
    const name = part.slice(0, separator).trim();
    try { result.set(name, decodeURIComponent(part.slice(separator + 1).trim())); }
    catch { /* Ignore malformed cookie values. */ }
  }
  return result;
}

function cookieName(request: Request, name: string): string {
  return new URL(request.url).protocol === "https:" ? `__Host-${name}` : name;
}

function cookieHeader(request: Request, name: string, value: string, maxAge: number, httpOnly: boolean, sameSite: "Lax" | "Strict"): string {
  const secure = new URL(request.url).protocol === "https:";
  return [
    `${cookieName(request, name)}=${encodeURIComponent(value)}`,
    "Path=/",
    `Max-Age=${Math.max(0, Math.floor(maxAge))}`,
    `SameSite=${sameSite}`,
    ...(httpOnly ? ["HttpOnly"] : []),
    ...(secure ? ["Secure"] : []),
  ].join("; ");
}

function clearedCookieHeaders(request: Request): string[] {
  return [ACCESS_COOKIE, REFRESH_COOKIE, CSRF_COOKIE].map(name =>
    cookieHeader(request, name, "", 0, name !== CSRF_COOKIE, name === CSRF_COOKIE ? "Strict" : "Lax"),
  );
}

function appendClearedCookies(headers: Headers, request: Request): void {
  for (const value of clearedCookieHeaders(request)) headers.append("Set-Cookie", value);
}

function csrfCookie(request: Request): string | undefined {
  return cookies(request).get(cookieName(request, CSRF_COOKIE));
}

function validateSameOriginMutation(request: Request): boolean {
  const origin = request.headers.get("origin");
  if (!origin || origin !== new URL(request.url).origin) return false;
  const fetchSite = request.headers.get("sec-fetch-site");
  if (fetchSite && fetchSite !== "same-origin") return false;
  const cookie = csrfCookie(request);
  const header = request.headers.get("x-csrf-token");
  return Boolean(cookie && header && cookie === header);
}

function apiOrigin(service: Service, request: Request): string {
  const env = getCloudflareContext().env as unknown as RuntimeEnv;
  const value = service === "identity" ? env.IDENTITY_API_ORIGIN
    : service === "journal" ? env.JOURNAL_API_ORIGIN
    : service === "market-data" ? env.MARKET_DATA_API_ORIGIN
    : env.API_GATEWAY_ORIGIN;
  const fallback = env.API_GATEWAY_ORIGIN;
  const origin = (value || fallback || "").trim().replace(/\/$/, "");
  const requestHost = new URL(request.url).hostname;
  const localDevelopment = requestHost === "localhost" || requestHost === "127.0.0.1";
  if (!origin || (!origin.startsWith("https://") && !(localDevelopment && origin.startsWith("http://")))) {
    throw new Error(`The ${service} API origin is not configured as an HTTPS origin.`);
  }
  return origin;
}

function isPublicRequest(service: Service, path: string, method: string): boolean {
  if (service === "market-data" && method === "GET") {
    return PUBLIC_MARKET_DATA_PATHS.includes(path);
  }
  return service === "identity" && method === "POST" &&
    (path === "/api/v1/auth/login" || path === "/api/v1/auth/register");
}

function responseWithCookies(response: Response, cookieHeaders: string[]): Response {
  const headers = new Headers();
  for (const name of ["content-type", "cache-control", "etag", "last-modified"]) {
    const value = response.headers.get(name);
    if (value) headers.set(name, value);
  }
  headers.set("Cache-Control", "no-store");
  for (const value of cookieHeaders) headers.append("Set-Cookie", value);
  return new Response(response.body, { status: response.status, headers });
}

async function refreshTokens(origin: string, refreshToken: string): Promise<AuthTokens | null> {
  const response = await fetch(`${origin}/api/v1/auth/refresh`, {
    method: "POST",
    headers: { "Content-Type": "application/json", Accept: "application/json" },
    body: JSON.stringify({ refresh_token: refreshToken }),
    redirect: "manual",
    cache: "no-store",
  });
  if (!response.ok) return null;
  const payload = await response.json() as Partial<AuthTokens>;
  if (!payload.access_token || !payload.refresh_token || !payload.expires_in || !payload.user) return null;
  return payload as AuthTokens;
}

function upstreamHeaders(request: Request, accessToken?: string): Headers {
  const headers = new Headers();
  for (const name of ["accept", "content-type", "x-organization-id", "x-workspace-id"]) {
    const value = request.headers.get(name);
    if (value) headers.set(name, value);
  }
  if (accessToken) headers.set("Authorization", `Bearer ${accessToken}`);
  return headers;
}

async function forward(request: Request, origin: string, path: string, accessToken?: string): Promise<Response> {
  const url = new URL(request.url);
  const target = `${origin}${path}${url.search}`;
  const method = request.method.toUpperCase();
  const body = SAFE_METHODS.has(method) ? undefined : await request.arrayBuffer();
  return fetch(target, {
    method,
    headers: upstreamHeaders(request, accessToken),
    body,
    redirect: "manual",
    cache: "no-store",
  });
}

export async function handleBffRequest(request: Request, serviceValue: string, pathParts: string[] = []): Promise<Response> {
  const method = request.method.toUpperCase();
  const jar = cookies(request);
  const accessToken = jar.get(cookieName(request, ACCESS_COOKIE));
  const refreshToken = jar.get(cookieName(request, REFRESH_COOKIE));

  if (serviceValue === "health" && pathParts.length === 0 && method === "GET") {
    try {
      const origin = apiOrigin("gateway", request);
      const health = await fetch(`${origin}/healthz`, {
        headers: { Accept: "text/plain" },
        redirect: "manual",
        cache: "no-store",
      });
      return responseWithCookies(health, []);
    } catch {
      return new Response("unavailable", { status: 503, headers: { "Cache-Control": "no-store" } });
    }
  }

  if (serviceValue === "csrf" && pathParts.length === 0 && method === "GET") {
    const token = csrfCookie(request) ?? crypto.randomUUID();
    const headers = new Headers({ "Cache-Control": "no-store" });
    if (!csrfCookie(request)) headers.append("Set-Cookie", cookieHeader(request, CSRF_COOKIE, token, 60 * 60 * 8, false, "Strict"));
    return new Response(null, { status: 204, headers });
  }

  if (!["identity", "journal", "market-data", "gateway"].includes(serviceValue)) {
    return Response.json({ error: { code: "invalid_bff_service", message: "Unknown API service." } }, { status: 404 });
  }
  const service = serviceValue as Service;
  if (pathParts.some(part => part === "." || part === ".." || part.includes("/") || part.includes("\\"))) {
    return Response.json({ error: { code: "invalid_bff_path", message: "Unknown API route." } }, { status: 404 });
  }
  const path = `/${pathParts.join("/")}`;
  if (service === "identity" && path === "/api/v1/auth/refresh") {
    return Response.json({ error: { code: "bff_managed_refresh", message: "Session refresh is managed by the secure BFF." } }, {
      status: 404,
      headers: { "Cache-Control": "no-store" },
    });
  }
  if (!path.startsWith("/api/v1/")) {
    return Response.json({ error: { code: "invalid_bff_path", message: "Unknown API route." } }, { status: 404 });
  }
  if (!SAFE_METHODS.has(method) && !validateSameOriginMutation(request)) {
    return Response.json({ error: { code: "csrf_validation_failed", message: "The request could not be verified. Refresh the page and try again." } }, { status: 403 });
  }

  const loginOrRegister = service === "identity" && method === "POST" &&
    (path === "/api/v1/auth/login" || path === "/api/v1/auth/register");
  const logoutRequest = service === "identity" && path === "/api/v1/auth/logout" && method === "POST";
  const publicRequest = isPublicRequest(service, path, method);

  if (logoutRequest) {
    if (refreshToken) {
      try {
        const identityOrigin = apiOrigin("identity", request);
        await fetch(`${identityOrigin}/api/v1/auth/logout`, {
          method: "POST",
          headers: { "Content-Type": "application/json", Accept: "application/json" },
          body: JSON.stringify({ refresh_token: refreshToken }),
          redirect: "manual",
          cache: "no-store",
          signal: AbortSignal.timeout(5_000),
        }).catch(() => undefined);
      } catch { /* Always clear the browser session, even if Identity is unavailable. */ }
    }
    const headers = new Headers({ "Cache-Control": "no-store" });
    appendClearedCookies(headers, request);
    return new Response(null, { status: 204, headers });
  }

  if (!loginOrRegister && !publicRequest && !accessToken && !refreshToken) {
    return Response.json({ error: { code: "unauthorized", message: "Sign in to continue." } }, { status: 401, headers: { "Cache-Control": "no-store" } });
  }

  let origin: string;
  try { origin = apiOrigin(service, request); }
  catch {
    return Response.json({ error: { code: "service_unavailable", message: "This service is temporarily unavailable." } }, { status: 503, headers: { "Cache-Control": "no-store" } });
  }

  if (loginOrRegister) {
    const upstream = await forward(request, origin, path);
    if (!upstream.ok) return responseWithCookies(upstream, []);
    const payload = await upstream.json() as Partial<AuthTokens>;
    if (!payload.access_token || !payload.refresh_token || !payload.expires_in || !payload.user) {
      return Response.json({ error: { code: "invalid_identity_response", message: "The identity service returned an invalid session." } }, { status: 502, headers: { "Cache-Control": "no-store" } });
    }
    const tokens = payload as AuthTokens;
    const headers = new Headers({ "Cache-Control": "no-store" });
    for (const name of [ACCESS_COOKIE, REFRESH_COOKIE]) {
      headers.append("Set-Cookie", cookieHeader(request, name, name === ACCESS_COOKIE ? tokens.access_token : tokens.refresh_token,
        name === ACCESS_COOKIE ? tokens.expires_in : 30 * 24 * 60 * 60, true, "Lax"));
    }
    return Response.json({ user: tokens.user }, { status: 200, headers });
  }
  let currentAccess = accessToken;
  let rotated: AuthTokens | null = null;
  if (!publicRequest && !currentAccess && refreshToken) {
    rotated = await refreshTokens(apiOrigin("identity", request), refreshToken).catch(() => null);
    currentAccess = rotated?.access_token;
    if (!currentAccess) {
      const headers = new Headers({ "Cache-Control": "no-store" });
      appendClearedCookies(headers, request);
      return Response.json({ error: { code: "unauthorized", message: "Your session has expired. Sign in again." } }, { status: 401, headers });
    }
  }

  let upstream = await forward(request, origin, path, publicRequest ? undefined : currentAccess);
  if (upstream.status === 401 && refreshToken && !rotated) {
    rotated = await refreshTokens(apiOrigin("identity", request), refreshToken).catch(() => null);
    if (rotated) upstream = await forward(request, origin, path, rotated.access_token);
  }

  const setCookies: string[] = [];
  if (rotated) {
    setCookies.push(cookieHeader(request, ACCESS_COOKIE, rotated.access_token, rotated.expires_in, true, "Lax"));
    setCookies.push(cookieHeader(request, REFRESH_COOKIE, rotated.refresh_token, 30 * 24 * 60 * 60, true, "Lax"));
  }
  if (upstream.status === 401) {
    return responseWithCookies(upstream, clearedCookieHeaders(request));
  }
  return responseWithCookies(upstream, setCookies);
}
