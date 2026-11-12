import { apiBaseUrl, apiRequest, requireApiMode } from "@/lib/api/client";
const journalBaseUrl = () => apiBaseUrl("journal");

export type JournalAnalyticsSavedView = {
  id: string;
  name: string;
  journal_account_id: string | null;
  filters: { fromDate?: string; toDate?: string; datePreset?: string; symbolFilter?: string; setupFilter?: string };
  updated_at: string;
};

export type JournalAccount = {
  id: string; name: string; broker?: string | null; account_type: string;
  currency: string; initial_balance: string; archived?: boolean;
};
export type JournalSetup = {
  id: string; name: string; description?: string | null; rules_json: Record<string, unknown>; archived?: boolean;
};
export type JournalRiskPolicy = {
  profile_name: string; firm_name: string | null; program_name: string | null;
  account_type: "personal" | "prop" | "challenge" | "funded" | "demo";
  risk_appetite: "conservative" | "balanced" | "aggressive" | "custom";
  risk_per_trade_pct: number; max_risk_per_trade_amount: number | null;
  max_daily_loss_pct: number | null; max_daily_risk_pct: number | null; daily_loss_basis: "initial_balance" | "start_of_day_balance" | "start_of_day_equity";
  max_total_drawdown_pct: number | null; drawdown_basis: "static_initial" | "trailing_high_water";
  max_trades_per_day: number | null; max_open_positions: number | null;
  max_total_open_risk_pct: number | null; minimum_rr: number | null;
  max_lot_size: number | null; max_leverage: number | null;
  firm_limits_enforcement: "block" | "warn"; personal_limits_enforcement: "block" | "warn";
  daily_reset_timezone: string; notes: string | null;
};
export type JournalRiskPolicyEnvelope = { policy: JournalRiskPolicy; version: number; updated_at: string | null; account_currency?: string };
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
  async listAnalyticsSavedViews(accountId?: string) {
    requireApiMode();
    const params = new URLSearchParams();
    if (accountId) params.set("journal_account_id", accountId);
    const suffix = params.toString() ? `?${params.toString()}` : "";
    const response = await apiRequest<DataEnvelope<JournalAnalyticsSavedView[]>>(`/api/v1/journal/analytics/saved-views${suffix}`, { baseUrl: journalBaseUrl() });
    return response.data.map(view => ({ id:view.id, name:view.name, accountId:view.journal_account_id ?? "", fromDate:view.filters.fromDate ?? "", toDate:view.filters.toDate ?? "", datePreset:view.filters.datePreset ?? "all", symbolFilter:view.filters.symbolFilter ?? "", setupFilter:view.filters.setupFilter ?? "" }));
  },
  async saveAnalyticsSavedView(input: { name:string; accountId:string | null; filters:{fromDate:string;toDate:string;datePreset:string;symbolFilter:string;setupFilter:string} }) {
    requireApiMode();
    return apiRequest<DataEnvelope<JournalAnalyticsSavedView>>("/api/v1/journal/analytics/saved-views", { method:"POST", baseUrl:journalBaseUrl(), body:JSON.stringify({name:input.name,journal_account_id:input.accountId,filters:input.filters}) });
  },
  async deleteAnalyticsSavedView(id:string) {
    requireApiMode();
    return apiRequest<void>(`/api/v1/journal/analytics/saved-views/${encodeURIComponent(id)}`, { method:"DELETE", baseUrl:journalBaseUrl() });
  },
  async getRiskPolicy(accountId: string) {
    requireApiMode();
    return apiRequest<{policy:JournalRiskPolicy;version:number;updated_at:string|null;account_currency?:string}>(`/api/v1/journal/accounts/${encodeURIComponent(accountId)}/risk-policy`, {baseUrl:journalBaseUrl()});
  },
  async saveRiskPolicy(accountId: string, policy: JournalRiskPolicy) {
    requireApiMode();
    return apiRequest<JournalRiskPolicyEnvelope>(`/api/v1/journal/accounts/${encodeURIComponent(accountId)}/risk-policy`, {method:"PUT",baseUrl:journalBaseUrl(),body:JSON.stringify(policy)});
  },
  async evaluateRiskPolicy(accountId: string, input: {currency:string;risk_amount:number;potential_reward?:number|null;equity:number;start_of_day_balance?:number;start_of_day_equity?:number;peak_equity?:number;lot_size?:number;trade_notional?:number;current_open_notional?:number}) {
    requireApiMode();
    return apiRequest<Record<string,unknown>>(`/api/v1/journal/accounts/${encodeURIComponent(accountId)}/risk-policy/evaluate`, {method:"POST",baseUrl:journalBaseUrl(),body:JSON.stringify(input)});
  },

  async saveExcursions(tradeId: string, input: {mfe_price:number;mae_price:number;mfe_r:number|null;mae_r:number|null;timeframe:string;provider:string;candle_count:number;coverage_start:string;coverage_end:string}) {
    requireApiMode();
    return apiRequest<DataEnvelope<{trade_id:string;saved:boolean;precision:string}>>(`/api/v1/journal/trades/${encodeURIComponent(tradeId)}/excursions`, {method:"PUT",baseUrl:journalBaseUrl(),body:JSON.stringify(input)});
  },
  async createAttachmentUploadUrl(tradeId: string, input: {original_filename:string;content_type:string;byte_size:number;attachment_type:"before"|"after"}) {
    requireApiMode();
    return (await apiRequest<DataEnvelope<{upload_url:string;storage_key:string;storage_provider:string;content_type:string;byte_size:number;original_filename:string;attachment_type:string;expires_in_seconds:number}>>(`/api/v1/journal/trades/${encodeURIComponent(tradeId)}/attachments/upload-url`,{method:"POST",baseUrl:journalBaseUrl(),body:JSON.stringify(input)})).data;
  },
  async getAttachmentDownloadUrl(tradeId:string, attachmentId:string) {
    requireApiMode();
    return (await apiRequest<DataEnvelope<{download_url:string;expires_in_seconds:number}>>(`/api/v1/journal/trades/${encodeURIComponent(tradeId)}/attachments/${encodeURIComponent(attachmentId)}/download-url`,{baseUrl:journalBaseUrl()})).data;
  },
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
  async analyticsSummary(options: { journal_account_id?: string; from?: string; to?: string; symbol?: string; setup_id?: string } = {}) {
    requireApiMode();
    const params = new URLSearchParams();
    if (options.journal_account_id) params.set("journal_account_id", options.journal_account_id);
    if (options.from) params.set("from", options.from);
    if (options.to) params.set("to", options.to);
    if (options.symbol) params.set("symbol", options.symbol);
    if (options.setup_id) params.set("setup_id", options.setup_id);
    const suffix = params.toString() ? `?${params.toString()}` : "";
    return (await apiRequest<DataEnvelope<JournalAnalytics>>(`/api/v1/journal/analytics/summary${suffix}`, { baseUrl: journalBaseUrl() })).data;
  },
  async analyticsIntelligence(options: { journal_account_id?: string; from?: string; to?: string; symbol?: string; setup_id?: string } = {}) {
    requireApiMode();
    const params = new URLSearchParams();
    if (options.journal_account_id) params.set("journal_account_id", options.journal_account_id);
    if (options.from) params.set("from", options.from);
    if (options.to) params.set("to", options.to);
    if (options.symbol) params.set("symbol", options.symbol);
    if (options.setup_id) params.set("setup_id", options.setup_id);
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
  async listTrades(options: { journal_account_id?: string; limit?: number; offset?: number; closed_from?: string; closed_to?: string; symbol?: string; setup_id?: string } = {}) {
    requireApiMode();
    const params = new URLSearchParams();
    if (options.journal_account_id) params.set("journal_account_id", options.journal_account_id);
    if (options.limit !== undefined) params.set("limit", String(options.limit));
    if (options.offset !== undefined) params.set("offset", String(options.offset));
    if (options.closed_from) params.set("closed_from", options.closed_from);
    if (options.closed_to) params.set("closed_to", options.closed_to);
    if (options.symbol) params.set("symbol", options.symbol);
    if (options.setup_id) params.set("setup_id", options.setup_id);
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
  generated_at?: string; mfe_coverage_count?: number; mae_coverage_count?: number;
  max_drawdown: string; average_r: string | null; average_mfe_r?: string | null; average_mae_r?: string | null;
  risk_coverage_count?: number; risk_coverage_percent?: string; average_risk_amount?: string | null; maximum_risk_amount?: string | null;
  average_holding_seconds?: number | null; median_holding_seconds?: number | null; maximum_win_streak?: number; maximum_loss_streak?: number; recovery_factor?: string | null; risk_distribution?: {minimum:string|null;q1:string|null;median:string|null;q3:string|null;maximum:string|null;sample_count:number};
  average_mfe_capture_percent?: string | null; execution_coverage_count?: number;
  trade_metrics?: Array<{trade_id:string;symbol:string;direction:"long"|"short";session?:string|null;setup?:string;opened_at:string;closed_at:string;net_pnl:string;risk_amount:string|null;realized_r:string|null;mfe_r:string|null;mae_r:string|null;mfe_source:string|null;mae_source:string|null;mfe_precision:string|null;mae_precision:string|null;holding_seconds:number;mfe_capture_percent:string|null;profit_surrendered_r:string|null;stop_loss_recorded:boolean;take_profit_recorded:boolean;entry_price:string;exit_price:string|null;setup_valid:boolean|null;rules_followed:boolean|null;emotional_trade:boolean|null;mistake:boolean|null;discipline_score:number|null;psychology:{emotion_before:string|null;emotion_during:string|null;emotion_after:string|null;confidence:number|null;fear:number|null;fomo:number|null;frustration:number|null;revenge_intensity:number|null;boredom:number|null}}>;
  setup_valid_count?: number; rules_followed_count?: number; emotional_trade_count?: number; mistake_count?: number; setup_valid_percent?: string | null; rules_followed_percent?: string | null; average_discipline_score?: string | null;
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
  psychology_record_count?: number; average_confidence?: number | null; average_fear?: number | null; average_fomo?: number | null; average_frustration?: number | null; average_revenge_intensity?: number | null; average_boredom?: number | null;
  emotion_before_record_count?: number; emotion_during_record_count?: number; emotion_after_record_count?: number;
  process_outcomes: { good_process_win: number; good_process_loss: number; poor_process_win: number; poor_process_loss: number };
  insights: Array<{ type: string; severity: "info" | "warning"; title: string; message: string }>;
  notice: string;
};
