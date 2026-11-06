"use client";

import { FormEvent, useCallback, useEffect, useState } from "react";
import { useAuth } from "@/features/auth/AuthProvider";

type JournalAccount = { id: string; name: string; currency: string; account_type: string };
type JournalTrade = {
  id: string; journal_account_id: string; symbol: string; direction: "long" | "short";
  status: "open" | "closed" | "cancelled"; opened_at: string; quantity: string;
  entry_price: string; exit_price?: string | null; net_pnl?: string | null; pnl_currency: string;
};

const initialDateTime = () => {
  const date = new Date();
  date.setMinutes(date.getMinutes() - date.getTimezoneOffset());
  return date.toISOString().slice(0, 16);
};

export default function JournalPage() {
  const { authFetch } = useAuth();
  const [accounts, setAccounts] = useState<JournalAccount[]>([]);
  const [trades, setTrades] = useState<JournalTrade[]>([]);
  const [accountId, setAccountId] = useState("");
  const [symbol, setSymbol] = useState("EURUSD");
  const [direction, setDirection] = useState<"long" | "short">("long");
  const [quantity, setQuantity] = useState("0.01");
  const [entryPrice, setEntryPrice] = useState("");
  const [openedAt, setOpenedAt] = useState(initialDateTime);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const request = useCallback(async (path: string, init: RequestInit = {}) => {
    const response = await authFetch(path, {
      ...init,
      headers: { "Content-Type": "application/json", ...(init.headers ?? {}) },
    });
    const data = await response.json().catch(() => ({}));
    if (!response.ok) throw new Error(data?.error?.message ?? "Journal request failed");
    return data;
  }, [authFetch]);

  const load = useCallback(async () => {
    setError("");
    try {
      const [accountResult, tradeResult] = await Promise.all([
        request("/api/v1/journal/accounts"),
        request("/api/v1/journal/trades?limit=250"),
      ]);
      const nextAccounts = (accountResult.data ?? []) as JournalAccount[];
      setAccounts(nextAccounts);
      setTrades((tradeResult.data ?? []) as JournalTrade[]);
      setAccountId((old) => old || nextAccounts[0]?.id || "");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Could not load journal records");
    } finally {
      setLoading(false);
    }
  }, [request]);

  useEffect(() => { void load(); }, [load]);

  async function createAccount() {
    setBusy(true); setError("");
    try {
      const result = await request("/api/v1/journal/accounts", {
        method: "POST",
        body: JSON.stringify({ name: "Personal Journal", broker: null, account_type: "journal", currency: "USD", initial_balance: "0" }),
      });
      const account = result.data as JournalAccount;
      setAccounts((old) => [account, ...old]);
      setAccountId(account.id);
    } catch (err) {
      setError(err instanceof Error ? err.message : "Account creation failed");
    } finally { setBusy(false); }
  }

  async function createTrade(event: FormEvent) {
    event.preventDefault();
    if (!accountId) { setError("Create or select a journal account first."); return; }
    setBusy(true); setError("");
    try {
      await request("/api/v1/journal/trades", {
        method: "POST",
        body: JSON.stringify({
          journal_account_id: accountId, setup_id: null, symbol: symbol.trim().toUpperCase(),
          direction, status: "open", source: "manual", source_system: null, source_trade_id: null,
          opened_at: new Date(openedAt).toISOString(), closed_at: null, quantity, entry_price: entryPrice,
          exit_price: null, stop_loss: null, take_profit: null, gross_pnl: null,
          commission: "0", swap: "0", other_fees: "0", net_pnl: null, pnl_currency: "USD",
          risk_amount: null, r_multiple: null,
        }),
      });
      setEntryPrice("");
      await load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Trade save failed");
    } finally { setBusy(false); }
  }

  async function deleteTrade(id: string) {
    if (!window.confirm("Delete this journal entry?")) return;
    try {
      await request(`/api/v1/journal/trades/${id}`, { method: "DELETE" });
      setTrades((old) => old.filter((trade) => trade.id !== id));
    } catch (err) {
      setError(err instanceof Error ? err.message : "Delete failed");
    }
  }

  if (loading) return <div className="loading">Loading your journal…</div>;

  return (
    <div>
      <header className="page-header">
        <div><h1>Trading Journal</h1><p>Personal trade records, separate from EdgeNest OMS and execution authority.</p></div>
        <span className="badge ok">Connected</span>
      </header>
      <div className="grid grid-kpi">
        <section className="panel"><div className="metric"><span className="metric-label">Journal trades</span><span className="metric-value">{trades.length}</span></div></section>
        <section className="panel"><div className="metric"><span className="metric-label">Open trades</span><span className="metric-value">{trades.filter((trade) => trade.status === "open").length}</span></div></section>
        <section className="panel"><div className="metric"><span className="metric-label">Journal accounts</span><span className="metric-value">{accounts.length}</span></div></section>
        <section className="panel"><div className="metric"><span className="metric-label">Data source</span><span className="metric-value">EdgeNest</span></div></section>
      </div>
      {error && <div className="error" role="alert" style={{ marginTop: 12 }}>{error}</div>}
      <section className="panel" style={{ marginTop: 14 }}>
        <div style={{ display: "flex", justifyContent: "space-between", gap: 12, alignItems: "center" }}>
          <h2 className="panel-title">JOURNAL ACCOUNT</h2>
          <button className="btn" disabled={busy} onClick={() => void createAccount()}>Create journal account</button>
        </div>
        {accounts.length ? (
          <label className="journal-field" style={{ maxWidth: 360 }}>
            <span>Account</span>
            <select value={accountId} onChange={(event) => setAccountId(event.target.value)}>
              {accounts.map((account) => <option key={account.id} value={account.id}>{account.name} · {account.currency}</option>)}
            </select>
          </label>
        ) : <p className="muted">Create an account before logging your first trade.</p>}
      </section>
      <section className="panel" style={{ marginTop: 14 }}>
        <h2 className="panel-title">LOG TRADE</h2>
        <form className="journal-form-grid" onSubmit={createTrade}>
          <label className="journal-field"><span>Symbol</span><input required value={symbol} onChange={(event) => setSymbol(event.target.value)} /></label>
          <label className="journal-field"><span>Direction</span><select value={direction} onChange={(event) => setDirection(event.target.value as "long" | "short")}><option value="long">Long</option><option value="short">Short</option></select></label>
          <label className="journal-field"><span>Quantity / lot size</span><input required type="number" min="0.00000001" step="any" value={quantity} onChange={(event) => setQuantity(event.target.value)} /></label>
          <label className="journal-field"><span>Entry price</span><input required type="number" min="0.00000001" step="any" value={entryPrice} onChange={(event) => setEntryPrice(event.target.value)} /></label>
          <label className="journal-field"><span>Opened at</span><input required type="datetime-local" value={openedAt} onChange={(event) => setOpenedAt(event.target.value)} /></label>
          <div style={{ display: "flex", alignItems: "end" }}><button className="btn primary" disabled={busy || !accountId}>{busy ? "Saving…" : "Save journal trade"}</button></div>
        </form>
      </section>
      <section className="panel" style={{ marginTop: 14 }}>
        <h2 className="panel-title">TRADE HISTORY</h2>
        {!trades.length ? <div className="journal-empty"><h3>No journal trades yet</h3><p>Log a trade above. Records will appear here after they are saved to EdgeNest.</p></div> : (
          <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Opened</th><th>Symbol</th><th>Side</th><th>Status</th><th>Qty</th><th>Entry</th><th>Exit</th><th>Net P&amp;L</th><th></th></tr></thead>
            <tbody>{trades.map((trade) => <tr key={trade.id}><td className="num">{new Date(trade.opened_at).toLocaleString()}</td><td>{trade.symbol}</td><td>{trade.direction.toUpperCase()}</td><td>{trade.status}</td><td className="num">{trade.quantity}</td><td className="num">{trade.entry_price}</td><td className="num">{trade.exit_price ?? "—"}</td><td className="num">{trade.net_pnl ?? "—"} {trade.net_pnl ? trade.pnl_currency : ""}</td><td><button className="btn" onClick={() => void deleteTrade(trade.id)}>Delete</button></td></tr>)}</tbody>
          </table></div>
        )}
      </section>
    </div>
  );
}
