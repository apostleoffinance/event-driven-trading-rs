import { apiBaseUrl, apiRequest, requireApiMode } from "@/lib/api/client";
const marketDataBaseUrl = () => apiBaseUrl("market-data");

export type MarketDataProvider = { id: string; status?: unknown; name?: string; [key: string]: unknown };

export function isProviderSelectable(provider: MarketDataProvider): boolean {
  if (!provider.status || typeof provider.status !== "object") return true;
  const state = String((provider.status as { state?: unknown }).state ?? "").toLowerCase();
  return !["disabled", "not_implemented", "unavailable"].includes(state);
}

export function providerStatusLabel(provider: MarketDataProvider): string {
  if (!provider.status || typeof provider.status !== "object") return "Status unavailable";
  const status = provider.status as { state?: unknown; reason?: unknown };
  const state = String(status.state ?? "unknown").toLowerCase();
  const labels: Record<string, string> = {
    available: "Available",
    demo: "Demo data",
    disabled: "Disabled",
    not_implemented: "Not supported yet",
    unavailable: "Unavailable",
  };
  const label = labels[state] ?? "Status unavailable";
  return typeof status.reason === "string" && status.reason.length ? `${label} — ${status.reason}` : label;
}
export type MarketInstrument = { id: string; symbol?: string; name?: string; [key: string]: unknown };
export type Candle = { timestamp_ms: number; open: number; high: number; low: number; close: number; volume?: number };
export type CandleResponse = { candles: Candle[]; count?: number; classification?: string; last_updated_at?: string; freshness?: unknown; [key: string]: unknown };

export const marketDataApi = {
  async providers() { requireApiMode(); return (await apiRequest<{ providers: MarketDataProvider[] }>("/api/v1/market-data/providers", { auth: false, baseUrl: marketDataBaseUrl() })).providers; },
  async instruments() { requireApiMode(); return (await apiRequest<{ instruments: MarketInstrument[] }>("/api/v1/market-data/instruments", { auth: false, baseUrl: marketDataBaseUrl() })).instruments; },
  async candles(input: { instrument: string; timeframe: string; limit?: number; provider?: string; from?: string; to?: string }) {
    requireApiMode();
    const params = new URLSearchParams({ instrument: input.instrument, timeframe: input.timeframe });
    if (input.limit !== undefined) params.set("limit", String(input.limit));
    if (input.provider) params.set("provider", input.provider);
    if (input.from) params.set("from", input.from);
    if (input.to) params.set("to", input.to);
    return apiRequest<CandleResponse>(`/api/v1/market-data/candles?${params.toString()}`, { auth: false, baseUrl: marketDataBaseUrl() });
  },
  async mt5Latest() { requireApiMode(); return apiRequest<unknown>("/api/v1/market-data/mt5/latest", { auth: false, baseUrl: marketDataBaseUrl() }); },
};
