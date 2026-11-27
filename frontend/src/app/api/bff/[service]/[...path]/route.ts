import { NextRequest, NextResponse } from "next/server";

export const dynamic = "force-dynamic";
export const runtime = "edge";

type Service = "identity" | "journal" | "market-data" | "gateway";
type Tokens = { access_token: string; refresh_token: string; expires_in: number; user?: unknown };

const ACCESS_COOKIE = "edgenest_access";
const REFRESH_COOKIE = "edgenest_refresh";
const CSRF_COOKIE = "edgenest_csrf";
const ACCESS_MAX_AGE = 15 * 60;
const REFRESH_MAX_AGE = 30 * 24 * 60 * 60;

function cookieValue(request: NextRequest, name: string): string | undefined {
  return request.cookies.get(name)?.value;
}
function cookieHeader(name: string, value: string, request: NextRequest, maxAge: number, httpOnly = true): string {
  const secure = request.nextUrl.protocol === "https:" ? "; Secure" : "";
  return `${name}=${encodeURIComponent(value)}; Path=/; Max-Age=${maxAge}; SameSite=Lax${httpOnly ? "; HttpOnly" : ""}${secure}`;
}
function clearCookie(name: string, request: NextRequest, httpOnly = true): string {
  return cookieHeader(name, "", request, 0, httpOnly);
}
function setTokenCookies(headers: Headers, tokens: Tokens, request: NextRequest) {
  headers.append("Set-Cookie", cookieHeader(ACCESS_COOKIE, tokens.access_token, request, Math.max(60, Math.min(tokens.expires_in, 3600))));
  headers.append("Set-Cookie", cookieHeader(REFRESH_COOKIE, tokens.refresh_token, request, REFRESH_MAX_AGE));
}
function clearTokenCookies(headers: Headers, request: NextRequest) {
  headers.append("Set-Cookie", clearCookie(ACCESS_COOKIE, request));
  headers.append("Set-Cookie", clearCookie(REFRESH_COOKIE, request));
}
function jsonError(status: number, code: string, message: string) {
  return NextResponse.json({ error: { code, message } }, { status, headers: { "Cache-Control": "no-store" } });
}

function serviceOrigin(service: Service): string | undefined {
  const values: Record<Service, string | undefined> = {
    identity: process.env.EDGENEST_IDENTITY_API_URL || process.env.NEXT_PUBLIC_IDENTITY_API_URL || process.env.NEXT_PUBLIC_API_URL,
    journal: process.env.EDGENEST_JOURNAL_API_URL || process.env.NEXT_PUBLIC_JOURNAL_API_URL || process.env.NEXT_PUBLIC_API_URL,
    "market-data": process.env.EDGENEST_MARKET_DATA_API_URL || process.env.NEXT_PUBLIC_MARKET_DATA_API_URL || process.env.NEXT_PUBLIC_API_URL,
    gateway: process.env.EDGENEST_API_URL || process.env.NEXT_PUBLIC_API_URL,
  };
  return values[service]?.trim().replace(/\/$/, "");
}

function allowedPath(service: Service, path: string): boolean {
  if (service === "identity") return path.startsWith("/api/v1/auth/");
  if (service === "journal") return path.startsWith("/api/v1/journal/");
  if (service === "market-data") return path.startsWith("/api/v1/market-data/");
  return path === "/healthz" || path === "/readyz" || path.startsWith("/api/v1/organizations") || path.startsWith("/api/v1/trading/");
}

function originAllowed(request: NextRequest): boolean {
  const origin = request.headers.get("origin");
  const fetchSite = request.headers.get("sec-fetch-site");
  if (fetchSite === "cross-site") return false;
  return !origin || origin === request.nextUrl.origin;
}
function csrfValid(request: NextRequest): boolean {
  const cookie = cookieValue(request, CSRF_COOKIE);
  const header = request.headers.get("x-csrf-token");
  return Boolean(cookie && header && cookie.length >= 32 && cookie === header);
}

async function upstreamFetch(url: string, request: NextRequest, accessToken?: string, body?: BodyInit): Promise<Response> {
  const headers = new Headers({ Accept: request.headers.get("accept") || "application/json" });
  const contentType = request.headers.get("content-type");
  if (contentType) headers.set("Content-Type", contentType);
  if (accessToken) headers.set("Authorization", `Bearer ${accessToken}`);
  const init: RequestInit = { method: request.method, headers, cache: "no-store", redirect: "manual" };
  if (body !== undefined && request.method !== "GET" && request.method !== "HEAD") init.body = body;
  return fetch(url, init);
}

async function rotateRefreshToken(origin: string, refreshToken: string): Promise<Tokens | null> {
  try {
    const response = await fetch(`${origin}/api/v1/auth/refresh`, {
      method: "POST",
      headers: { Accept: "application/json", "Content-Type": "application/json" },
      body: JSON.stringify({ refresh_token: refreshToken }),
      cache: "no-store",
      redirect: "manual",
    });
    if (!response.ok) return null;
    const payload = await response.json() as Tokens;
    return payload.access_token && payload.refresh_token ? payload : null;
  } catch {
    return null;
  }
}

async function handle(request: NextRequest, context: { params: Promise<{ service: string; path: string[] }> }) {
  if (!originAllowed(request)) return jsonError(403, "cross_origin_request", "Cross-origin requests are not allowed.");
  const { service: rawService, path: segments } = await context.params;
  if (!["identity", "journal", "market-data", "gateway"].includes(rawService)) return jsonError(404, "service_not_found", "Unknown API service.");
  const service = rawService as Service;
  const path = `/${segments.join("/")}`;
  if (!allowedPath(service, path)) return jsonError(404, "route_not_allowed", "This route is not available through the application gateway.");
  const origin = serviceOrigin(service);
  if (!origin) return jsonError(503, "service_not_configured", "This service is temporarily unavailable.");
  const method = request.method.toUpperCase();
  if (!["GET", "HEAD", "POST", "PUT", "PATCH", "DELETE"].includes(method)) return jsonError(405, "method_not_allowed", "This method is not supported.");
  const mutating = !["GET", "HEAD"].includes(method);
  if (mutating && request.headers.get("origin") !== request.nextUrl.origin) return jsonError(403, "origin_check_failed", "A same-origin request is required.");
  if (mutating && !csrfValid(request)) return jsonError(403, "csrf_failed", "Your session security check failed. Refresh the page and try again.");

  const url = `${origin}${path}${request.nextUrl.search}`;
  const body = mutating ? await request.arrayBuffer() : undefined;
  const isLoginOrRegister = service === "identity" && (path.endsWith("/auth/login") || path.endsWith("/auth/register")) && method === "POST";
  const isRefresh = service === "identity" && path.endsWith("/auth/refresh") && method === "POST";
  const isLogout = service === "identity" && path.endsWith("/auth/logout") && method === "POST";
  const refreshCookie = cookieValue(request, REFRESH_COOKIE);
  let accessToken = cookieValue(request, ACCESS_COOKIE);
  let response: Response;

  if (isRefresh) {
    if (!refreshCookie) {
      const headers = new Headers({ "Cache-Control": "no-store" });
      clearTokenCookies(headers, request);
      return new Response(JSON.stringify({ error: { code: "unauthorized", message: "Your session has expired. Please sign in again." } }), { status: 401, headers: { ...Object.fromEntries(headers), "Content-Type": "application/json" } });
    }
    const tokens = await rotateRefreshToken(origin, refreshCookie);
    if (!tokens) {
      const headers = new Headers({ "Content-Type": "application/json", "Cache-Control": "no-store" });
      clearTokenCookies(headers, request);
      return new Response(JSON.stringify({ error: { code: "unauthorized", message: "Your session has expired. Please sign in again." } }), { status: 401, headers });
    }
    const headers = new Headers({ "Content-Type": "application/json", "Cache-Control": "no-store" });
    setTokenCookies(headers, tokens, request);
    return new Response(JSON.stringify({ user: tokens.user }), { status: 200, headers });
  }

  if (isLoginOrRegister) {
    response = await upstreamFetch(url, request, undefined, body);
    if (!response.ok) return new Response(response.body, { status: response.status, headers: { "Content-Type": response.headers.get("content-type") || "application/json", "Cache-Control": "no-store" } });
    const tokens = await response.json() as Tokens;
    if (!tokens.access_token || !tokens.refresh_token || !tokens.user) return jsonError(502, "invalid_identity_response", "Identity service returned an invalid session.");
    const headers = new Headers({ "Content-Type": "application/json", "Cache-Control": "no-store" });
    setTokenCookies(headers, tokens, request);
    return new Response(JSON.stringify({ user: tokens.user }), { status: response.status, headers });
  }

  if (isLogout) {
    if (!accessToken && refreshCookie) {
      const rotated = await rotateRefreshToken(serviceOrigin("identity") || origin, refreshCookie);
      accessToken = rotated?.access_token;
    }
    response = await upstreamFetch(url, request, accessToken, JSON.stringify({ refresh_token: refreshCookie || "" }));
    if (response.status === 401 && refreshCookie) {
      const rotated = await rotateRefreshToken(serviceOrigin("identity") || origin, refreshCookie);
      if (rotated) response = await upstreamFetch(url, request, rotated.access_token, JSON.stringify({ refresh_token: rotated.refresh_token }));
    }
    const headers = new Headers({ "Cache-Control": "no-store" });
    clearTokenCookies(headers, request);
    return new Response(null, { status: response.ok || response.status === 401 ? 204 : response.status, headers });
  }

  response = await upstreamFetch(url, request, accessToken, body);
  let rotated: Tokens | null = null;
  if (response.status === 401 && refreshCookie && !isLoginOrRegister && !isRefresh && !isLogout) {
    rotated = await rotateRefreshToken(serviceOrigin("identity") || origin, refreshCookie);
    if (rotated) {
      accessToken = rotated.access_token;
      response = await upstreamFetch(url, request, accessToken, body);
    }
  }
  const headers = new Headers();
  const contentType = response.headers.get("content-type");
  if (contentType) headers.set("Content-Type", contentType);
  headers.set("Cache-Control", "no-store");
  if (rotated) setTokenCookies(headers, rotated, request);
  if (response.status === 401) clearTokenCookies(headers, request);
  return new Response(response.body, { status: response.status, headers });
}

export const GET = handle;
export const HEAD = handle;
export const POST = handle;
export const PUT = handle;
export const PATCH = handle;
export const DELETE = handle;
