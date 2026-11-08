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
  risk_amount?: string | null; r_multiple?: string | null; session?: string | null; timeframe?: string | null; timezone?: string | null; mfe_r?: string | null; mae_r?: string | null; notes?: string | null; tags?: string[]; setup_valid?: boolean | null; rules_followed?: boolean | null; emotional_trade?: boolean | null; mistake?: boolean | null; mistake_notes?: string | null; discipline_score?: number | null; acknowledged_warnings?: string[];
};
type DataEnvelope<T> = { data: T };
export type PaginatedTrades = DataEnvelope<JournalTrade[]> & { pagination: { limit: number; offset: number } };

export const journalApi = {
  async listAttachments(tradeId: string) {
    requireApiMode(); return (await apiRequest<DataEnvelope<JournalAttachment[]>>(`/api/v1/journal/trades/${encodeURIComponent(tradeId)}/attachments`,{baseUrl:journalBaseUrl()})).data;
  },
  async createAttachment(tradeId: string,input:Omit<JournalAttachment,"id">) {
    requireApiMode(); return apiRequest<DataEnvelope<{id:string;trade_id:string;saved:boolean}>>(`/api/v1/journal/trades/${encodeURIComponent(tradeId)}/attachments`,{method:"POST",baseUrl:journalBaseUrl(),body:JSON.stringify(input)});
  },
  async deleteAttachment(tradeId:string,id:string) {
    requireApiMode(); return apiRequest<void>(`/api/v1/journal/trades/${encodeURIComponent(tradeId)}/attachments/${encodeURIComponent(id)}`,{method:"DELETE",baseUrl:journalBaseUrl()});
  },
  async getPsychology(tradeId: string) {
    requireApiMode();
    return (await apiRequest<DataEnvelope<JournalPsychology | null>>(`/api/v1/journal/trades/${encodeURIComponent(tradeId)}/psychology`, { baseUrl: journalBaseUrl() })).data;
  },
  async savePsychology(tradeId: string, input: JournalPsychology) {
    requireApiMode();
    return apiRequest<DataEnvelope<{trade_id:string;saved:boolean}>>(`/api/v1/journal/trades/${encodeURIComponent(tradeId)}/psychology`, { method:"PUT", baseUrl:journalBaseUrl(), body:JSON.stringify(input) });
  },
  async analyticsSummary(options: { journal_account_id?: string; from?: string; to?: string } = {}) {
    requireApiMode();
    const params = new URLSearchParams();
    if (options.journal_account_id) params.set("journal_account_id", options.journal_account_id);
    if (options.from) params.set("from", options.from);
    if (options.to) params.set("to", options.to);
    const suffix = params.toString() ? `?${params.toString()}` : "";
    return (await apiRequest<DataEnvelope<JournalAnalytics>>(`/api/v1/journal/analytics/summary${suffix}`, { baseUrl: journalBaseUrl() })).data;
  },
  async analyticsIntelligence(options: { journal_account_id?: string; from?: string; to?: string } = {}) {
    requireApiMode();
    const params = new URLSearchParams();
    if (options.journal_account_id) params.set("journal_account_id", options.journal_account_id);
    if (options.from) params.set("from", options.from);
    if (options.to) params.set("to", options.to);
    const suffix = params.toString() ? `?${params.toString()}` : "";
    return (await apiRequest<DataEnvelope<JournalIntelligence>>(`/api/v1/journal/analytics/intelligence${suffix}`, { baseUrl: journalBaseUrl() })).data;
  },
  async listAccounts() { requireApiMode(); return (await apiRequest<DataEnvelope<JournalAccount[]>>("/api/v1/journal/accounts", { baseUrl: journalBaseUrl() })).data; },
  async updateAccount(id: string, input: { name: string; broker?: string | null; account_type: string; currency: string; initial_balance: string }) {
    requireApiMode(); return apiRequest<void>(`/api/v1/journal/accounts/${encodeURIComponent(id)}`, { method: "PUT", baseUrl: journalBaseUrl(), body: JSON.stringify(input) });
  },
  async archiveAccount(id: string) {
    requireApiMode(); return apiRequest<void>(`/api/v1/journal/accounts/${encodeURIComponent(id)}`, { method: "DELETE", baseUrl: journalBaseUrl() });
  },
  async createAccount(input: { name: string; broker?: string; account_type?: string; currency?: string; initial_balance?: string }) {
    requireApiMode(); return (await apiRequest<DataEnvelope<JournalAccount>>("/api/v1/journal/accounts", { method: "POST", baseUrl: journalBaseUrl(), body: JSON.stringify(input) })).data;
  },
  async listSetups() { requireApiMode(); return (await apiRequest<DataEnvelope<JournalSetup[]>>("/api/v1/journal/setups", { baseUrl: journalBaseUrl() })).data; },
  async updateSetup(id: string, input: { name: string; description?: string | null; rules_json?: Record<string, unknown> }) {
    requireApiMode(); return apiRequest<void>(`/api/v1/journal/setups/${encodeURIComponent(id)}`, { method: "PUT", baseUrl: journalBaseUrl(), body: JSON.stringify(input) });
  },
  async archiveSetup(id: string) {
    requireApiMode(); return apiRequest<void>(`/api/v1/journal/setups/${encodeURIComponent(id)}`, { method: "DELETE", baseUrl: journalBaseUrl() });
  },
  async createSetup(input: { name: string; description?: string; rules_json?: Record<string, unknown> }) {
    requireApiMode(); return (await apiRequest<DataEnvelope<JournalSetup>>("/api/v1/journal/setups", { method: "POST", baseUrl: journalBaseUrl(), body: JSON.stringify(input) })).data;
  },
  async importTrades(input: { journal_account_id: string; source_system: string; trades: Omit<JournalTrade, "id">[] }) {
    requireApiMode();
    return apiRequest<DataEnvelope<{ batch_id: string; source_count: number; imported_count: number; skipped_count: number; failed_count: number; status: string }>>("/api/v1/journal/import", { method: "POST", baseUrl: journalBaseUrl(), body: JSON.stringify(input) }).then(x=>x.data);
  },
  async listTrades(options: { journal_account_id?: string; limit?: number; offset?: number } = {}) {
    requireApiMode();
    const params = new URLSearchParams();
    if (options.journal_account_id) params.set("journal_account_id", options.journal_account_id);
    if (options.limit !== undefined) params.set("limit", String(options.limit));
    if (options.offset !== undefined) params.set("offset", String(options.offset));
    const query = params.toString();
    const suffix = query ? `?${query}` : "";
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

export type JournalAnalytics = {
  trade_count: number; wins: number; losses: number; breakeven: number;
  win_rate_percent: string; gross_profit: string; gross_loss: string; net_profit: string;
  profit_factor: string | null; expectancy: string; average_win: string | null; average_loss: string | null;
  max_drawdown: string; average_r: string | null; average_mfe_r?: string | null; average_mae_r?: string | null; setup_valid_count?: number; rules_followed_count?: number; emotional_trade_count?: number; mistake_count?: number; setup_valid_percent?: string | null; rules_followed_percent?: string | null; average_discipline_score?: string | null;
  equity_curve: Array<{ trade_id: string; closed_at: string; net_pnl: string; equity: string; drawdown: string }>;
  by_symbol: Array<{ key: string; trades: number; net_pnl: string; win_rate_percent: string; expectancy: string }>;
  by_session: Array<{ key: string; trades: number; net_pnl: string; win_rate_percent: string; expectancy: string }>;
  by_setup?: Array<{ key: string; trades: number; net_pnl: string; win_rate_percent: string; expectancy: string }>;
  by_weekday?: Array<{ key: string; trades: number; net_pnl: string; win_rate_percent: string; expectancy: string }>;
  by_hour_utc?: Array<{ key: string; trades: number; net_pnl: string; win_rate_percent: string; expectancy: string }>;
  by_day?: Array<{ key: string; trades: number; net_pnl: string; win_rate_percent: string; expectancy: string }>;
  by_month?: Array<{ key: string; trades: number; net_pnl: string; win_rate_percent: string; expectancy: string }>;
  notice?: string;
};

export type JournalPsychology = {
  emotion_before?: string | null; emotion_during?: string | null; emotion_after?: string | null;
  confidence?: number | null; fear?: number | null; fomo?: number | null; frustration?: number | null;
  revenge_intensity?: number | null; boredom?: number | null; notes?: string | null;
};

export type JournalAttachment = { id:string; attachment_type:string; storage_provider:string; storage_key:string; original_filename?:string|null; content_type?:string|null; byte_size?:number|null; captured_at?:string|null; metadata:Record<string,unknown> };

export type JournalIntelligence = {
  trade_count: number; wins: number; losses: number; breakeven: number;
  win_rate_percent: string | null;
  win_rate_confidence_95: { lower_percent: number; upper_percent: number } | null;
  sample_status: "insufficient_sample" | "early_sample" | "more_stable_sample";
  minimum_sample_recommendation: number;
  rule_adherence_percent: number | null; rule_adherence_known_count: number;
  valid_setup_percent: number | null; valid_setup_known_count: number;
  emotional_trade_count: number; mistake_count: number; average_discipline_score: number | null;
  process_outcomes: { good_process_win: number; good_process_loss: number; poor_process_win: number; poor_process_loss: number };
  insights: Array<{ type: string; severity: "info" | "warning"; title: string; message: string }>;
  notice: string;
};
