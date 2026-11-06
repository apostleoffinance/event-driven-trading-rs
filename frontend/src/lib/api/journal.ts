import { apiBaseUrl, apiRequest, requireApiMode } from "@/lib/api/client";
const journalBaseUrl = () => apiBaseUrl("journal");

export type JournalAccount = {
  id: string; name: string; broker?: string | null; account_type: string;
  currency: string; initial_balance: string; archived?: boolean;
};
export type JournalSetup = {
  id: string; name: string; description?: string | null; rules_json: Record<string, unknown>; archived?: boolean;
};
export type JournalTrade = {
  id: string; journal_account_id: string; setup_id?: string | null; symbol: string;
  direction: "long" | "short"; status: string; source: string; source_system?: string | null;
  source_trade_id?: string | null; opened_at: string; closed_at?: string | null;
  quantity: string; entry_price: string; exit_price?: string | null; stop_loss?: string | null;
  take_profit?: string | null; gross_pnl?: string | null; commission: string; swap: string;
  other_fees: string; net_pnl?: string | null; pnl_currency: string;
  risk_amount?: string | null; r_multiple?: string | null;
};
type DataEnvelope<T> = { data: T };
export type PaginatedTrades = DataEnvelope<JournalTrade[]> & { pagination: { limit: number; offset: number } };

export const journalApi = {
  async listAccounts() { requireApiMode(); return (await apiRequest<DataEnvelope<JournalAccount[]>>("/api/v1/journal/accounts", { baseUrl: journalBaseUrl() })).data; },
  async createAccount(input: { name: string; broker?: string; account_type?: string; currency?: string; initial_balance?: string }) {
    requireApiMode(); return (await apiRequest<DataEnvelope<JournalAccount>>("/api/v1/journal/accounts", { method: "POST", baseUrl: journalBaseUrl(), body: JSON.stringify(input) })).data;
  },
  async listSetups() { requireApiMode(); return (await apiRequest<DataEnvelope<JournalSetup[]>>("/api/v1/journal/setups", { baseUrl: journalBaseUrl() })).data; },
  async createSetup(input: { name: string; description?: string; rules_json?: Record<string, unknown> }) {
    requireApiMode(); return (await apiRequest<DataEnvelope<JournalSetup>>("/api/v1/journal/setups", { method: "POST", baseUrl: journalBaseUrl(), body: JSON.stringify(input) })).data;
  },
  async listTrades(options: { journal_account_id?: string; limit?: number; offset?: number } = {}) {
    requireApiMode();
    const params = new URLSearchParams();
    if (options.journal_account_id) params.set("journal_account_id", options.journal_account_id);
    if (options.limit !== undefined) params.set("limit", String(options.limit));
    if (options.offset !== undefined) params.set("offset", String(options.offset));
    const query = params.toString();\n    const suffix = query ? `?${query}` : "";
    return apiRequest<PaginatedTrades>(`/api/v1/journal/trades${suffix}`, { baseUrl: journalBaseUrl() });
  },
  async createTrade(input: Omit<JournalTrade, "id">) {
    requireApiMode(); return (await apiRequest<DataEnvelope<JournalTrade>>("/api/v1/journal/trades", { method: "POST", baseUrl: journalBaseUrl(), body: JSON.stringify(input) })).data;
  },
  async getTrade(id: string) { requireApiMode(); return (await apiRequest<DataEnvelope<JournalTrade>>(`/api/v1/journal/trades/${encodeURIComponent(id)}`, { baseUrl: journalBaseUrl() })).data; },
  async updateTrade(id: string, input: Omit<JournalTrade, "id">) {
    requireApiMode(); return apiRequest<void>(`/api/v1/journal/trades/${encodeURIComponent(id)}`, { method: "PUT", baseUrl: journalBaseUrl(), body: JSON.stringify(input) });
  },
  async deleteTrade(id: string) { requireApiMode(); return apiRequest<void>(`/api/v1/journal/trades/${encodeURIComponent(id)}`, { method: "DELETE", baseUrl: journalBaseUrl() }); },
};
