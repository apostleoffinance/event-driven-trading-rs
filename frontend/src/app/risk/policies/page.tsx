"use client";

import { useEffect, useMemo, useState, type FormEvent } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import Link from "next/link";
import { journalApi, type JournalRiskPolicy } from "@/lib/api/journal";

type PolicyForm = Record<keyof JournalRiskPolicy, string>;
const defaultForm: PolicyForm = {
  profile_name:"My risk policy",firm_name:"",program_name:"",account_type:"personal",risk_appetite:"balanced",
  risk_per_trade_pct:"0.5",max_risk_per_trade_amount:"",max_daily_loss_pct:"",max_daily_risk_pct:"",daily_loss_basis:"start_of_day_equity",
  max_total_drawdown_pct:"",drawdown_basis:"trailing_high_water",max_trades_per_day:"",max_open_positions:"",
  max_total_open_risk_pct:"",minimum_rr:"1",max_lot_size:"",max_leverage:"",
  firm_limits_enforcement:"block",personal_limits_enforcement:"warn",daily_reset_timezone:"Africa/Lagos",notes:"",
};
function textValue(value: unknown): string { return value == null ? "" : String(value); }
function formFromPolicy(policy: JournalRiskPolicy): PolicyForm {
  return Object.fromEntries(Object.keys(defaultForm).map(key=>[key,textValue(policy[key as keyof JournalRiskPolicy])])) as PolicyForm;
}
function optionalNumber(value: string): number | null {
  if (!value.trim()) return null;
  const n=Number(value);
  return Number.isFinite(n) ? n : null;
}
function number(value: string): number {
  const n=Number(value);
  return Number.isFinite(n) ? n : 0;
}

export default function RiskPoliciesPage() {
  const client=useQueryClient();
  const accounts=useQuery({queryKey:["journal","accounts"],queryFn:journalApi.listAccounts});
  const [accountId,setAccountId]=useState("");
  const [form,setForm]=useState<PolicyForm>(defaultForm);
  const [saving,setSaving]=useState(false);
  const [saved,setSaved]=useState(false);
  const [error,setError]=useState<string|null>(null);
  const [loadedVersion,setLoadedVersion]=useState<number>(0);
  const selected=useMemo(()=>accounts.data?.find(a=>a.id===accountId),[accounts.data,accountId]);
  const policyQuery=useQuery({queryKey:["journal","risk-policy",accountId],queryFn:()=>journalApi.getRiskPolicy(accountId),enabled:Boolean(accountId)});

  useEffect(()=>{ if(!accountId&&accounts.data?.length) setAccountId(accounts.data[0].id); },[accountId,accounts.data]);
  useEffect(()=>{
    if(policyQuery.data){setForm(formFromPolicy(policyQuery.data.policy));setLoadedVersion(policyQuery.data.version);setSaved(false);}
  },[policyQuery.data]);

  function update<K extends keyof PolicyForm>(key:K,value:PolicyForm[K]){setForm(current=>({...current,[key]:value}));setSaved(false);setError(null);}
  async function save(event:FormEvent<HTMLFormElement>){
    event.preventDefault();if(!accountId)return;
    setSaving(true);setSaved(false);setError(null);
    const payload:JournalRiskPolicy={
      profile_name:form.profile_name.trim(),firm_name:form.firm_name.trim()||null,program_name:form.program_name.trim()||null,
      account_type:form.account_type as JournalRiskPolicy["account_type"],risk_appetite:form.risk_appetite as JournalRiskPolicy["risk_appetite"],
      risk_per_trade_pct:number(form.risk_per_trade_pct),max_risk_per_trade_amount:optionalNumber(form.max_risk_per_trade_amount),
      max_daily_loss_pct:optionalNumber(form.max_daily_loss_pct),max_daily_risk_pct:optionalNumber(form.max_daily_risk_pct),daily_loss_basis:form.daily_loss_basis as JournalRiskPolicy["daily_loss_basis"],
      max_total_drawdown_pct:optionalNumber(form.max_total_drawdown_pct),drawdown_basis:form.drawdown_basis as JournalRiskPolicy["drawdown_basis"],
      max_trades_per_day:optionalNumber(form.max_trades_per_day),max_open_positions:optionalNumber(form.max_open_positions),
      max_total_open_risk_pct:optionalNumber(form.max_total_open_risk_pct),minimum_rr:optionalNumber(form.minimum_rr),
      max_lot_size:optionalNumber(form.max_lot_size),max_leverage:optionalNumber(form.max_leverage),
      firm_limits_enforcement:form.firm_limits_enforcement as JournalRiskPolicy["firm_limits_enforcement"],
      personal_limits_enforcement:form.personal_limits_enforcement as JournalRiskPolicy["personal_limits_enforcement"],
      daily_reset_timezone:form.daily_reset_timezone.trim(),notes:form.notes.trim()||null,
    };
    try{
      const result=await journalApi.saveRiskPolicy(accountId,payload);
      setForm(formFromPolicy(result.policy));setLoadedVersion(result.version);setSaved(true);
      await client.invalidateQueries({queryKey:["journal","risk-policy",accountId]});
    }catch(e){setError(e instanceof Error?e.message:"Unable to save risk policy.");}
    finally{setSaving(false);}
  }

  if(accounts.isLoading)return <div className="loading">Loading Journal accounts…</div>;
  if(accounts.isError)return <div className="error">Unable to load accounts. Check the Journal API connection.</div>;

  return <main>
    <header className="page-header"><div><h1>Account Risk Policies</h1><p>Configure independent personal risk and prop-firm rules for each trading account. Risk policy is account-scoped.</p></div><Link className="btn" href="/risk">Risk Center</Link></header>
    {!accounts.data?.length?<section className="panel"><h2>No Journal accounts yet</h2><p>Create a Journal account first. Each account can have its own firm, challenge, funded-account, or personal risk policy.</p><Link className="btn primary" href="/journal">Open Journal</Link></section>:<>
      <section className="panel" style={{marginBottom:12}}>
        <label className="journal-field"><span>Account</span><select value={accountId} onChange={e=>{setAccountId(e.target.value);setSaved(false);}}>{accounts.data.map(a=><option key={a.id} value={a.id}>{a.name} · {a.account_type} · {a.currency} {a.initial_balance}</option>)}</select></label>
        {selected?<p className="muted" style={{marginTop:8}}>Account currency: <strong>{selected.currency}</strong> · Starting balance: <strong>{selected.initial_balance}</strong></p>:null}
      </section>
      {policyQuery.isLoading?<div className="loading">Loading account policy…</div>:policyQuery.isError?<div className="error">Could not load this account’s risk policy.</div>:<form onSubmit={save} className="grid">
        <section className="panel"><h2 className="panel-title">PROFILE & RISK APPETITE</h2><div className="journal-form-grid">
          <label className="journal-field"><span>Policy name</span><input value={form.profile_name} onChange={e=>update("profile_name",e.target.value)} required maxLength={120}/></label>
          <label className="journal-field"><span>Prop firm (optional)</span><input value={form.firm_name} onChange={e=>update("firm_name",e.target.value)} placeholder="e.g. your firm name" maxLength={120}/></label>
          <label className="journal-field"><span>Program / account stage</span><input value={form.program_name} onChange={e=>update("program_name",e.target.value)} placeholder="e.g. 2-step challenge / funded" maxLength={120}/></label>
          <label className="journal-field"><span>Account type</span><select value={form.account_type} onChange={e=>update("account_type",e.target.value)}>{["personal","prop","challenge","funded","demo"].map(x=><option key={x}>{x}</option>)}</select></label>
          <label className="journal-field"><span>Risk appetite</span><select value={form.risk_appetite} onChange={e=>update("risk_appetite",e.target.value)}>{["conservative","balanced","aggressive","custom"].map(x=><option key={x}>{x}</option>)}</select></label>
          <label className="journal-field"><span>Risk per trade (% equity)</span><input type="number" min="0.001" max="100" step="0.001" value={form.risk_per_trade_pct} onChange={e=>update("risk_per_trade_pct",e.target.value)} required/></label>
          <label className="journal-field"><span>Absolute per-trade risk cap ({selected?.currency})</span><input type="number" min="0.01" step="0.01" value={form.max_risk_per_trade_amount} onChange={e=>update("max_risk_per_trade_amount",e.target.value)} placeholder="Optional"/></label>
          <label className="journal-field"><span>Minimum reward:risk</span><input type="number" min="0.01" step="0.01" value={form.minimum_rr} onChange={e=>update("minimum_rr",e.target.value)} placeholder="Optional"/></label>
        </div><p className="muted">Risk percentage and absolute risk cap are both applied. The effective cap is the stricter limit.</p></section>
        <section className="panel"><h2 className="panel-title">PROP-FIRM / ACCOUNT LIMITS</h2><div className="journal-form-grid">
          <label className="journal-field"><span>Max daily loss (%)</span><input type="number" min="0.001" max="100" step="0.001" value={form.max_daily_loss_pct} onChange={e=>update("max_daily_loss_pct",e.target.value)} placeholder="Optional"/></label>
          <label className="journal-field"><span>Max daily risk budget (% equity)</span><input type="number" min="0.001" max="100" step="0.001" value={form.max_daily_risk_pct} onChange={e=>update("max_daily_risk_pct",e.target.value)} placeholder="Optional"/></label><label className="journal-field"><span>Daily loss basis</span><select value={form.daily_loss_basis} onChange={e=>update("daily_loss_basis",e.target.value)}><option value="initial_balance">Initial account balance</option><option value="start_of_day_balance">Start-of-day balance</option><option value="start_of_day_equity">Start-of-day equity</option></select></label>
          <label className="journal-field"><span>Max total drawdown (%)</span><input type="number" min="0.001" max="100" step="0.001" value={form.max_total_drawdown_pct} onChange={e=>update("max_total_drawdown_pct",e.target.value)} placeholder="Optional"/></label>
          <label className="journal-field"><span>Drawdown basis</span><select value={form.drawdown_basis} onChange={e=>update("drawdown_basis",e.target.value)}><option value="static_initial">Static initial balance</option><option value="trailing_high_water">Trailing high-water mark</option></select></label>
          <label className="journal-field"><span>Max trades / day</span><input type="number" min="1" step="1" value={form.max_trades_per_day} onChange={e=>update("max_trades_per_day",e.target.value)} placeholder="Optional"/></label>
          <label className="journal-field"><span>Max simultaneous positions</span><input type="number" min="1" step="1" value={form.max_open_positions} onChange={e=>update("max_open_positions",e.target.value)} placeholder="Optional"/></label>
          <label className="journal-field"><span>Max total open risk (% equity)</span><input type="number" min="0.001" max="100" step="0.001" value={form.max_total_open_risk_pct} onChange={e=>update("max_total_open_risk_pct",e.target.value)} placeholder="Optional"/></label>
          <label className="journal-field"><span>Maximum lot size</span><input type="number" min="0.001" step="0.001" value={form.max_lot_size} onChange={e=>update("max_lot_size",e.target.value)} placeholder="Optional"/></label>
          <label className="journal-field"><span>Max leverage (x)</span><input type="number" min="0.01" step="0.01" value={form.max_leverage} onChange={e=>update("max_leverage",e.target.value)} placeholder="Optional"/></label>
          <label className="journal-field"><span>Daily reset timezone</span><input value={form.daily_reset_timezone} onChange={e=>update("daily_reset_timezone",e.target.value)} placeholder="Africa/Lagos" required/></label>
        </div><p className="muted">Daily P&amp;L and trade counts are derived from the account Journal ledger using the selected timezone. The backend does not assume every prop firm uses the same reset time or drawdown calculation.</p></section>
        <section className="panel"><h2 className="panel-title">ENFORCEMENT & NOTES</h2><div className="journal-form-grid">
          <label className="journal-field"><span>Firm-rule violations</span><select value={form.firm_limits_enforcement} onChange={e=>update("firm_limits_enforcement",e.target.value)}><option value="block">Block / hard warning</option><option value="warn">Warning only</option></select></label>
          <label className="journal-field"><span>Personal preference violations</span><select value={form.personal_limits_enforcement} onChange={e=>update("personal_limits_enforcement",e.target.value)}><option value="warn">Warning only</option><option value="block">Block / hard warning</option></select></label>
          <label className="journal-field" style={{gridColumn:"1 / -1"}}><span>Firm rules / custom notes</span><textarea rows={4} maxLength={2000} value={form.notes} onChange={e=>update("notes",e.target.value)} placeholder="Record exact rule wording, consistency rules, news restrictions, weekend rules, etc."/></label>
        </div></section>
        {error?<div className="error" role="alert">{error}</div>:null}
        {saved?<div className="panel" role="status"><strong>Policy saved and reloaded.</strong> Version {loadedVersion} · backend persistence confirmed.</div>:null}
        <div style={{display:"flex",gap:10,alignItems:"center"}}><button className="btn primary" type="submit" disabled={saving}>{saving?"Saving…":"Save account risk policy"}</button><span className="muted">Changes apply to future calculator assessments.</span></div>
      </form>}
    </>}
  </main>;
}
