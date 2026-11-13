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
const accountTypes = ["personal","prop","challenge","funded","demo"];
const riskAppetites = [{value:"conservative",label:"Conservative",detail:"0.25% guide"},{value:"balanced",label:"Balanced",detail:"0.50% guide"},{value:"aggressive",label:"Aggressive",detail:"1.00% guide"},{value:"custom",label:"Custom",detail:"Your own limit"}];
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
function formatMoney(value:number,currency:string) {
  try { return new Intl.NumberFormat(undefined,{style:"currency",currency,maximumFractionDigits:2}).format(value); }
  catch { return value.toFixed(2)+" "+currency; }
}
function validAccountType(value:string): value is JournalRiskPolicy["account_type"] {
  return accountTypes.includes(value);
}

function Field({label,hint,children,wide=false}:{label:string;hint?:string;children:React.ReactNode;wide?:boolean}) {
  return <label className={"risk-field"+(wide?" risk-field-wide":"")}><span className="risk-field-label">{label}</span>{children}{hint?<small>{hint}</small>:null}</label>;
}
function Section({number:step,title,description,children}:{number:string;title:string;description:string;children:React.ReactNode}) {
  return <section className="panel risk-section">
    <div className="risk-section-heading"><span className="risk-step">{step}</span><div><h2>{title}</h2><p>{description}</p></div></div>
    {children}
  </section>;
}
function Metric({label,value,sub}:{label:string;value:string;sub?:string}) {
  return <div className="risk-metric"><span>{label}</span><strong>{value}</strong>{sub?<small>{sub}</small>:null}</div>;
}

export default function RiskPoliciesPage() {
  const client=useQueryClient();
  const accounts=useQuery({queryKey:["journal","accounts"],queryFn:journalApi.listAccounts});
  const [accountId,setAccountId]=useState("");
  const [accountSearch,setAccountSearch]=useState("");
  const [form,setForm]=useState<PolicyForm>(defaultForm);
  const [baseline,setBaseline]=useState(JSON.stringify(defaultForm));
  const [saving,setSaving]=useState(false);
  const [saved,setSaved]=useState(false);
  const [error,setError]=useState<string|null>(null);
  const [loadedVersion,setLoadedVersion]=useState<number>(0);
  const selected=useMemo(()=>accounts.data?.find(a=>a.id===accountId),[accounts.data,accountId]);
  const filteredAccounts=useMemo(()=>accounts.data?.filter(a=>a.name.toLowerCase().includes(accountSearch.toLowerCase())||a.broker?.toLowerCase().includes(accountSearch.toLowerCase()))??[],[accounts.data,accountSearch]);
  const policyQuery=useQuery({queryKey:["journal","risk-policy",accountId],queryFn:()=>journalApi.getRiskPolicy(accountId),enabled:Boolean(accountId)});
  const dirty=JSON.stringify(form)!==baseline;
  const balance=Number(selected?.initial_balance??0);
  const pctRisk=balance>0?balance*number(form.risk_per_trade_pct)/100:0;
  const absoluteCap=optionalNumber(form.max_risk_per_trade_amount);
  const effectiveRisk=absoluteCap==null?pctRisk:Math.min(pctRisk,absoluteCap);
  const dailyLoss=optionalNumber(form.max_daily_loss_pct);
  const drawdown=optionalNumber(form.max_total_drawdown_pct);
  const checks=[
    {ok:Boolean(form.profile_name.trim()),label:"Policy name is set"},
    {ok:number(form.risk_per_trade_pct)>0&&number(form.risk_per_trade_pct)<=100,label:"Per-trade risk is valid"},
    {ok:Boolean(form.daily_reset_timezone.trim()),label:"Daily reset timezone is set"},
    {ok:dailyLoss!=null,label:"Daily loss limit configured"},
    {ok:drawdown!=null,label:"Maximum drawdown configured"},
  ];
  const missingCount=checks.filter(x=>!x.ok).length;

  useEffect(()=>{ if(!accountId&&accounts.data?.length) setAccountId(accounts.data[0].id); },[accountId,accounts.data]);
  useEffect(()=>{
    if(policyQuery.data){
      const next=formFromPolicy(policyQuery.data.policy);
      // New policies inherit the Journal account classification where it maps
      // to a supported policy type; existing saved policy values remain authoritative.
      if(policyQuery.data.version===0&&selected&&validAccountType(selected.account_type)) next.account_type=selected.account_type;
      setForm(next);setBaseline(JSON.stringify(next));setLoadedVersion(policyQuery.data.version);setSaved(false);setError(null);
    }
  },[policyQuery.data,selected]);

  function update<K extends keyof PolicyForm>(key:K,value:PolicyForm[K]){setForm(current=>({...current,[key]:value}));setSaved(false);setError(null);}
  function switchAccount(nextId:string) {
    if(nextId===accountId)return;
    if(dirty&&!window.confirm("You have unsaved policy changes. Discard them and switch accounts?"))return;
    setAccountId(nextId);setSaved(false);setError(null);
  }
  function validateForm():string|null {
    if(!form.profile_name.trim())return "Enter a name for this risk policy.";
    if(number(form.risk_per_trade_pct)<=0||number(form.risk_per_trade_pct)>100)return "Risk per trade must be greater than 0 and no more than 100%.";
    if(absoluteCap!=null&&absoluteCap<=0)return "The absolute per-trade risk cap must be greater than zero.";
    if(dailyLoss!=null&&(dailyLoss<=0||dailyLoss>100))return "Maximum daily loss must be between 0 and 100%.";
    if(drawdown!=null&&(drawdown<=0||drawdown>100))return "Maximum drawdown must be between 0 and 100%.";
    if(optionalNumber(form.max_daily_risk_pct)!=null&&(number(form.max_daily_risk_pct)<=0||number(form.max_daily_risk_pct)>100))return "Daily risk budget must be between 0 and 100%.";
    if(optionalNumber(form.max_total_open_risk_pct)!=null&&(number(form.max_total_open_risk_pct)<=0||number(form.max_total_open_risk_pct)>100))return "Maximum open risk must be between 0 and 100%.";
    if(!form.daily_reset_timezone.trim())return "Enter a valid IANA timezone, for example Africa/Lagos.";
    return null;
  }
  async function save(event:FormEvent<HTMLFormElement>){
    event.preventDefault();if(!accountId)return;
    const validation=validateForm();if(validation){setError(validation);return;}
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
      const next=formFromPolicy(result.policy);setForm(next);setBaseline(JSON.stringify(next));setLoadedVersion(result.version);setSaved(true);
      await client.invalidateQueries({queryKey:["journal","risk-policy",accountId]});
    }catch(e){setError(e instanceof Error?e.message:"Unable to save risk policy.");}
    finally{setSaving(false);}
  }
  function discardChanges(){if(!dirty)return;const parsed=JSON.parse(baseline) as PolicyForm;setForm(parsed);setError(null);setSaved(false);}

  if(accounts.isLoading)return <div className="loading">Loading Journal accounts…</div>;
  if(accounts.isError)return <div className="error">Unable to load accounts. Check the Journal API connection.</div>;

  return <main className="risk-policy-page">
    <header className="risk-page-header">
      <div><div className="risk-eyebrow">RISK MANAGEMENT / CONFIGURATION</div><h1>Account Risk Policies</h1><p>Set clear risk boundaries for each trading account. Review how your limits translate into real monetary exposure before saving.</p></div>
      <Link className="btn" href="/risk">Risk Center <span aria-hidden="true">↗</span></Link>
    </header>
    {!accounts.data?.length?<section className="panel risk-empty"><div className="risk-empty-icon">◎</div><h2>No Journal accounts yet</h2><p>Create or import a Journal account before configuring its risk policy. Each account has an independent policy.</p><Link className="btn primary" href="/journal">Open Journal</Link></section>:<>
      <section className="panel risk-account-card">
        <div className="risk-account-top"><div><div className="risk-eyebrow">POLICY ACCOUNT</div><h2>{selected?.name??"Select an account"}</h2><p>{selected?.broker||"Journal account"} · {selected?.account_type??"Account"} · {selected?.currency??"—"}</p></div><span className={"risk-state-pill "+(loadedVersion>0?"configured":"new")}>{loadedVersion>0?"Saved policy · v"+loadedVersion:"New policy"}</span></div>
        <div className="risk-account-controls"><label className="risk-field"><span className="risk-field-label">Find an account</span><input value={accountSearch} onChange={e=>setAccountSearch(e.target.value)} placeholder="Search by account or broker…" /></label><label className="risk-field"><span className="risk-field-label">Trading account</span><select value={accountId} onChange={e=>switchAccount(e.target.value)}>{filteredAccounts.map(a=><option key={a.id} value={a.id}>{a.name} · {a.currency}</option>)}</select></label></div>
        <div className="risk-account-facts"><Metric label="ACCOUNT CURRENCY" value={selected?.currency??"—"}/><Metric label="STARTING CAPITAL" value={selected?formatMoney(balance,selected.currency):"—"}/><Metric label="ACCOUNT TYPE" value={selected?.account_type??"—"}/></div>
      </section>
      {policyQuery.isLoading?<div className="loading">Loading account policy…</div>:policyQuery.isError?<div className="error">Could not load this account’s risk policy. Your existing configuration has not been changed.</div>:<form onSubmit={save} className="risk-policy-layout">
        <div className="risk-policy-main">
          <Section number="01" title="Policy profile" description="Identify the account ruleset and choose your personal risk approach.">
            <div className="risk-form-grid">
              <Field label="Policy name" hint="Give this configuration a name you can recognize later."><input value={form.profile_name} onChange={e=>update("profile_name",e.target.value)} required maxLength={120} placeholder="e.g. Funded account — conservative"/></Field>
              <Field label="Account classification" hint="Used to describe the account; it does not connect or verify firm rules."><select value={form.account_type} onChange={e=>update("account_type",e.target.value)}>{accountTypes.map(x=><option key={x} value={x}>{x.charAt(0).toUpperCase()+x.slice(1)}</option>)}</select></Field>
              <Field label="Prop firm / provider" hint="Optional. Enter the firm associated with this account."><input value={form.firm_name} onChange={e=>update("firm_name",e.target.value)} placeholder="e.g. FundedNext" maxLength={120}/></Field>
              <Field label="Program or account stage"><input value={form.program_name} onChange={e=>update("program_name",e.target.value)} placeholder="e.g. Stellar Instant · Funded" maxLength={120}/></Field>
            </div>
            <div className="risk-subheading">Risk appetite</div>
            <div className="risk-appetite-options">{riskAppetites.map(option=><button key={option.value} type="button" className={"risk-appetite-option"+(form.risk_appetite===option.value?" active":"")} onClick={()=>update("risk_appetite",option.value)} aria-pressed={form.risk_appetite===option.value}><strong>{option.label}</strong><span>{option.detail}</span></button>)}</div>
            <p className="risk-help">Risk appetite is a descriptive profile. Your explicit risk-per-trade value below remains the value used in assessments.</p>
          </Section>

          <Section number="02" title="Trade risk limits" description="Set the maximum planned loss and the minimum reward you require for each trade.">
            <div className="risk-form-grid">
              <Field label="Risk per trade (% of equity)" hint="Applied to the equity supplied to the assessment."><div className="risk-input-suffix"><input type="number" min="0.001" max="100" step="0.001" value={form.risk_per_trade_pct} onChange={e=>update("risk_per_trade_pct",e.target.value)} required/><span>%</span></div></Field>
              <Field label={"Absolute risk cap ("+(selected?.currency??"account currency")+")"} hint="Optional fixed monetary ceiling. The stricter of this and the percentage limit applies."><div className="risk-input-suffix"><input type="number" min="0.01" step="0.01" value={form.max_risk_per_trade_amount} onChange={e=>update("max_risk_per_trade_amount",e.target.value)} placeholder="No fixed cap"/><span>{selected?.currency??"—"}</span></div></Field>
              <Field label="Minimum reward-to-risk" hint="For example, 2 means the planned reward must be at least twice the risk."><div className="risk-input-suffix"><input type="number" min="0.01" max="100" step="0.01" value={form.minimum_rr} onChange={e=>update("minimum_rr",e.target.value)} placeholder="Optional"/><span>R</span></div></Field>
            </div>
            <div className="risk-calculation-card"><div><span>Estimated risk at starting capital</span><strong>{formatMoney(pctRisk,selected?.currency??"USD")}</strong></div><div><span>Effective per-trade cap</span><strong>{formatMoney(effectiveRisk,selected?.currency??"USD")}</strong></div><small>Illustrative only. The assessment uses its supplied equity, not necessarily the starting balance shown here.</small></div>
          </Section>

          <Section number="03" title="Account protection" description="Configure loss boundaries, drawdown rules and exposure limits.">
            <div className="risk-subheading">Loss protection</div>
            <div className="risk-form-grid">
              <Field label="Maximum daily loss (%)" hint="The daily loss ceiling for this account."><div className="risk-input-suffix"><input type="number" min="0.001" max="100" step="0.001" value={form.max_daily_loss_pct} onChange={e=>update("max_daily_loss_pct",e.target.value)} placeholder="Not set"/><span>%</span></div></Field>
              <Field label="Daily loss calculation basis"><select value={form.daily_loss_basis} onChange={e=>update("daily_loss_basis",e.target.value)}><option value="initial_balance">Initial account balance</option><option value="start_of_day_balance">Start-of-day balance</option><option value="start_of_day_equity">Start-of-day equity</option></select></Field>
              <Field label="Maximum total drawdown (%)" hint="The maximum decline from the selected reference point."><div className="risk-input-suffix"><input type="number" min="0.001" max="100" step="0.001" value={form.max_total_drawdown_pct} onChange={e=>update("max_total_drawdown_pct",e.target.value)} placeholder="Not set"/><span>%</span></div></Field>
              <Field label="Drawdown calculation basis"><select value={form.drawdown_basis} onChange={e=>update("drawdown_basis",e.target.value)}><option value="static_initial">Static initial balance</option><option value="trailing_high_water">Trailing high-water mark</option></select></Field>
            </div>
            <div className="risk-subheading">Exposure & trading activity</div>
            <div className="risk-form-grid">
              <Field label="Daily risk budget (% equity)" hint="Total planned risk across trades opened during the reset day."><div className="risk-input-suffix"><input type="number" min="0.001" max="100" step="0.001" value={form.max_daily_risk_pct} onChange={e=>update("max_daily_risk_pct",e.target.value)} placeholder="No limit"/><span>%</span></div></Field>
              <Field label="Maximum total open risk (% equity)"><div className="risk-input-suffix"><input type="number" min="0.001" max="100" step="0.001" value={form.max_total_open_risk_pct} onChange={e=>update("max_total_open_risk_pct",e.target.value)} placeholder="No limit"/><span>%</span></div></Field>
              <Field label="Maximum trades per day"><input type="number" min="1" step="1" value={form.max_trades_per_day} onChange={e=>update("max_trades_per_day",e.target.value)} placeholder="No limit"/></Field>
              <Field label="Maximum simultaneous positions"><input type="number" min="1" step="1" value={form.max_open_positions} onChange={e=>update("max_open_positions",e.target.value)} placeholder="No limit"/></Field>
              <Field label="Maximum lot size"><input type="number" min="0.001" step="0.001" value={form.max_lot_size} onChange={e=>update("max_lot_size",e.target.value)} placeholder="No limit"/></Field>
              <Field label="Maximum leverage"><div className="risk-input-suffix"><input type="number" min="0.01" step="0.01" value={form.max_leverage} onChange={e=>update("max_leverage",e.target.value)} placeholder="No limit"/><span>×</span></div></Field>
              <Field label="Daily reset timezone" hint="Use an IANA timezone such as Africa/Lagos."><input value={form.daily_reset_timezone} onChange={e=>update("daily_reset_timezone",e.target.value)} placeholder="Africa/Lagos" required/></Field>
            </div>
            <p className="risk-help">Daily P&amp;L and trade counts are derived from the Journal ledger using this timezone. Broker equity, floating P&amp;L and high-water marks are not assumed to be available from Journal history.</p>
          </Section>

          <Section number="04" title="Rules & assessment behaviour" description="Keep firm restrictions distinct from your personal trading preferences.">
            <div className="risk-rule-callout"><strong>Firm rules and personal rules are different.</strong><span>Only enter firm limits you have checked against the current rules for your exact program. QuantOS does not provide verified firm presets in this workspace.</span></div>
            <div className="risk-form-grid">
              <Field label="Firm-rule violations" hint="Applied to configured firm restrictions during calculator/Journal assessment."><select value={form.firm_limits_enforcement} onChange={e=>update("firm_limits_enforcement",e.target.value)}><option value="block">Block calculator assessment</option><option value="warn">Warn only</option></select></Field>
              <Field label="Personal preference violations" hint="Applied to your own risk preferences."><select value={form.personal_limits_enforcement} onChange={e=>update("personal_limits_enforcement",e.target.value)}><option value="warn">Warning only</option><option value="block">Block calculator assessment</option></select></Field>
              <Field label="Firm rules, exceptions & notes" hint="Record source wording, news restrictions, weekend rules or reset-time details." wide><textarea rows={4} maxLength={2000} value={form.notes} onChange={e=>update("notes",e.target.value)} placeholder="Add the exact rule wording and where you verified it…"/></Field>
            </div>
            <div className="risk-scope-note"><span aria-hidden="true">ⓘ</span><p>These policies are used for calculator and Journal assessments only. Saving a policy does not block MT5 orders, control a broker account or submit trades.</p></div>
          </Section>
          {error?<div className="error" role="alert">{error}</div>:null}
          {saved?<div className="risk-success" role="status"><strong>Policy saved successfully</strong><span>Version {loadedVersion} · The saved policy has been reloaded from the Journal API.</span></div>:null}
          <div className="risk-form-bottom-spacer"/>
        </div>

        <aside className="risk-policy-aside">
          <div className="panel risk-preview">
            <div className="risk-preview-head"><div><div className="risk-eyebrow">LIVE CONFIGURATION</div><h2>Policy summary</h2></div><span className={"risk-state-pill "+(missingCount===0?"configured":"new")}>{missingCount===0?"Ready to save":"Needs review"}</span></div>
            <div className="risk-preview-balance"><span>Starting account capital</span><strong>{selected?formatMoney(balance,selected.currency):"—"}</strong></div>
            <div className="risk-preview-grid">
              <Metric label="RISK PER TRADE" value={form.risk_per_trade_pct+"%"} sub={formatMoney(effectiveRisk,selected?.currency??"USD")+" effective cap"}/>
              <Metric label="MINIMUM REWARD:RISK" value={form.minimum_rr?number(form.minimum_rr).toFixed(2)+"R":"—"}/>
              <Metric label="DAILY LOSS LIMIT" value={dailyLoss==null?"Not set":dailyLoss+"%"}/>
              <Metric label="MAX DRAWDOWN" value={drawdown==null?"Not set":drawdown+"%"}/>
            </div>
            <div className="risk-preview-checks"><div className="risk-subheading">Configuration checks</div>{checks.map(item=><div className="risk-check" key={item.label}><span className={item.ok?"check-ok":"check-pending"}>{item.ok?"✓":"!"}</span><span>{item.label}</span></div>)}</div>
            <div className="risk-preview-foot"><span>Policy version</span><strong>{loadedVersion>0?"v"+loadedVersion:"Not saved"}</strong></div>
            {dirty?<div className="risk-dirty-note">You have unsaved changes.</div>:null}
          </div>
          <div className="risk-aside-help"><strong>How policy assessment works</strong><p>Firm restrictions and personal preferences are assessed separately. The calculator uses the account policy together with the trade inputs and available Journal history.</p><Link href="/risk">View Risk Center ↗</Link></div>
        </aside>

        <div className="risk-savebar">
          <div className="risk-save-status"><span className={dirty?"risk-unsaved-dot":"risk-saved-dot"}/><div><strong>{dirty?"Unsaved changes":"All changes saved"}</strong><small>{dirty?"Review the summary and save when ready.":"Your current policy matches the saved version."}</small></div></div>
          <div className="risk-save-actions"><button type="button" className="btn" onClick={discardChanges} disabled={!dirty||saving}>Discard</button><button type="submit" className="btn primary" disabled={!dirty||saving}>{saving?"Saving policy…":"Save policy"}</button></div>
        </div>
      </form>}
    </>}
  </main>;
}
