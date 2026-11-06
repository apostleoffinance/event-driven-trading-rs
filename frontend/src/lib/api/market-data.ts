import { apiBaseUrl, apiRequest, requireApiMode } from "@/lib/api/client";
const marketDataBaseUrl = () => apiBaseUrl("market-data");

export type MarketDataProvider = { id: string; status?: string; name?: string; [key: string]: unknown };
export type MarketInstrument = { id: string; symbol?: string; name?: string; [key: string]: unknown };
export type Candle = { timestamp?: string | number; time?: string | number; open: string | number; high: string | number; low: string | number; close: string | number; volume?: string | number };
export type CandleResponse = { candles: Candle[]; [key: string]: unknown };

export const marketDataApi = {
  async providers() { requireApiMode(); return (await apiRequest<{ providers: MarketDataProvider[] }>("/api/v1/market-data/providers", { auth: false, baseUrl: marketDataBaseUrl() })).providers; },
  async instruments() { requireApiMode(); return (await apiRequest<{ instruments: MarketInstrument[] }>("/api/v1/market-data/instruments", { auth: false })).instruments; },
  async candles(input: { instrument: string; timeframe: string; limit?: number; provider?: string; from?: string; to?: string }) {
    requireApiMode();
    const params = new URLSearchParams({ instrument: input.instrument, timeframe: input.timeframe });
    if (input.limit !== undefined) params.set("limit", String(input.limit));
    if (input.provider) params.set("provider", input.provider);
    if (input.from) params.set("from", input.from);
    if (input.to) params.set("to", input.to);
    return apiRequest<CandleResponse>(`/api/v1/market-data/candles?${params.toString()}`, { auth: false });
  },
  async mt5Latest() { requireApiMode(); return apiRequest<unknown>("/api/v1/market-data/mt5/latest", { auth: false }); },
};
