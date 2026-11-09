"use client";
import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { journalApi } from "@/lib/api/journal";
import { hasSession } from "@/lib/auth/session";

type Mode = "fixed_risk_sl" | "risk_to_levels" | "entry_sl_to_size" | "trade_analysis" | "target_distance";
const presets = [
 {symbol:"EURUSD", label:"EURUSD · FX", contract:100000, step:0.01, pip:0.0001, entry:"1.08500", sl:"1.08000"},
 {symbol:"GBPUSD", label:"GBPUSD · FX", contract:100000, step:0.01, pip:0.0001, entry:"1.27000", sl:"1.26500"},
 {symbol:"USDJPY", label:"USDJPY · FX (JPY quote)", contract:100000, step:0.01, pip:0.01, entry:"155.000", sl:"154.500"},
 {symbol:"XAUUSD", label:"XAUUSD · Gold", contract:100, step:0.01, pip:0.01, entry:"2700", sl:"2695"},
 {symbol:"BTCUSD", label:"BTCUSD · Crypto", contract:1, step:0.001, pip:1, entry:"80000", sl:"79000"},
 {symbol:"US100", label:"US100 · Index CFD", contract:1, step:1, pip:1, entry:"20000", sl:"19950"},
 {symbol:"CUSTOM", label:"Custom broker instrument", contract:1, step:0.01, pip:0.01, entry:"100", sl:"99"},
];
const modes: {id:Mode;name:string;hint:string}[] = [
 {id:"fixed_risk_sl",name:"Risk + SL → size",hint:"Calculate size from risk and stop"},
 {id:"risk_to_levels",name:"Risk → SL / TP",hint:"Calculate levels from money risk/reward"},
 {id:"entry_sl_to_size",name:"Entry + SL → size",hint:"Position size from entry/stop"},
 {id:"trade_analysis",name:"Trade analysis",hint:"Analyze risk, reward and R:R"},
 {id:"target_distance",name:"Target profit → TP",hint:"Calculate target from desired profit"},
];
const num=(v:string)=>Number(v);
const fmtMoney=(v:number,c:string)=>v.toLocaleString("en-US",{style:"currency",currency:c,maximumFractionDigits:2});
const roundDown=(v:number,step:number)=>Math.floor(v/step+1e-10)*step;

export default function TradeCalculator(){
 const [auth,setAuth]=useState(false);
 const accounts=useQuery({queryKey:["journal","accounts"],queryFn:journalApi.listAccounts,enabled:auth});
 useEffect(()=>setAuth(hasSession()),[]);
 const [mode,setMode]=useState<Mode>("fixed_risk_sl"),[symbol,setSymbol]=useState("EURUSD"),[direction,setDirection]=useState<"long"|"short">("long");
 const [account,setAccount]=useState(""),[currency,setCurrency]=useState("USD"),[balance,setBalance]=useState("10000"),[riskPct,setRiskPct]=useState("1");
 const [entry,setEntry]=useState("1.08500"),[stop,setStop]=useState("1.08000"),[target,setTarget]=useState("1.09500"),[qty,setQty]=useState("0.20");
 const [risk,setRisk]=useState("100"),[reward,setReward]=useState("200"),[contract,setContract]=useState("100000"),[step,setStep]=useState("0.01"),[pip,setPip]=useState("0.0001");
 useEffect(()=>{const a=accounts.data?.find(x=>x.id===account);if(a){setCurrency(a.currency.toUpperCase());setBalance(a.initial_balance);}},[account,accounts.data]);
 const selectInstrument=(s:string)=>{const p=presets.find(x=>x.symbol===s)!;setSymbol(s);setContract(String(p.contract));setStep(String(p.step));setPip(String(p.pip));setEntry(p.entry);setStop(p.sl);};
 const result=useMemo(()=>{
  const e=num(entry),sl=num(stop),tp=num(target),q=num(qty),b=num(balance),rp=num(riskPct),r=num(risk),rw=num(reward),cv=num(contract),st=num(step),ps=num(pip);
  if(![e,b,cv,st,ps].every(Number.isFinite)||e<=0||b<=0||cv<=0||st<=0||ps<=0)return {error:"Entry, balance, contract value, quantity step and pip size must be positive."};
  if(mode==="fixed_risk_sl"||mode==="entry_sl_to_size"){
   if(!Number.isFinite(sl)||sl<=0||(direction==="long"?sl>=e:sl<=e))return {error:"Stop-loss must be below entry for Long and above entry for Short."};
   if(rp<=0||rp>100)return {error:"Risk percentage must be greater than 0 and no more than 100."};
   const budget=b*rp/100,dist=Math.abs(e-sl),raw=budget/(dist*cv),size=roundDown(raw,st),actualRisk=size*dist*cv;
   if(size<=0)return {error:"Risk budget is below minimum volume step. Increase risk or reduce step."};
   return {size,budget,dist,pips:dist/ps,raw,risk:actualRisk,riskPct:actualRisk/b*100};
  }
  if(mode==="risk_to_levels"){
   if(!Number.isFinite(q)||q<=0||r<=0||rw<=0)return {error:"Quantity, risk amount and target reward must be positive."};
   const slDist=r/(q*cv),tpDist=rw/(q*cv);
   return {size:q,budget:r,dist:slDist,pips:slDist/ps,risk:r,reward:rw,rr:rw/r,stopPrice:e+(direction==="long"?-1:1)*slDist,targetPrice:e+(direction==="long"?1:-1)*tpDist};
  }
  if(!Number.isFinite(q)||q<=0)return {error:"Position quantity must be positive."};
  if(mode==="target_distance"){
   if(rw<=0)return {error:"Target reward must be positive."};
   const dist=rw/(q*cv);return {size:q,dist,pips:dist/ps,targetPrice:e+(direction==="long"?1:-1)*dist,reward:rw};
  }
  if(!Number.isFinite(sl)||!Number.isFinite(tp)||sl<=0||tp<=0||(direction==="long"?(sl>=e||tp<=e):(sl<=e||tp>=e)))return {error:"For Long, SL must be below entry and TP above. For Short, reverse these levels."};
  const dist=Math.abs(e-sl),targetDist=Math.abs(tp-e),estimatedRisk=q*dist*cv,potentialReward=q*targetDist*cv;
  return {size:q,dist,pips:dist/ps,risk:estimatedRisk,reward:potentialReward,riskPct:estimatedRisk/b*100,rr:potentialReward/estimatedRisk};
 },[mode,entry,stop,target,qty,balance,riskPct,risk,reward,contract,step,pip,direction]);
 const stopValue=result&&!('error' in result)&&('stopPrice' in result)?result.stopPrice:stop;
 const targetValue=result&&!('error' in result)&&('targetPrice' in result)?result.targetPrice:target;
 const journalLink=result&&!('error' in result)&&('size' in result)?"/journal?newTrade=1&symbol="+encodeURIComponent(symbol)+"&direction="+direction+"&entry="+encodeURIComponent(entry)+"&stop="+encodeURIComponent(String(stopValue))+"&quantity="+encodeURIComponent(String(result.size))+"&risk="+encodeURIComponent(String(("risk" in result ? result.risk : "")))+"&target="+encodeURIComponent(String(targetValue))+"&account="+encodeURIComponent(account)+"&currency="+encodeURIComponent(currency):"/journal";
 const needQty=mode==="risk_to_levels"||mode==="trade_analysis"||mode==="target_distance";
 const needSL=mode!=="risk_to_levels"&&mode!=="target_distance";
 const needTP=mode==="trade_analysis";
 const needRiskPct=mode==="fixed_risk_sl"||mode==="entry_sl_to_size";
 const needRiskMoney=mode==="risk_to_levels";
 const needReward=mode==="risk_to_levels"||mode==="target_distance";
 return <div>
  <header className="page-header"><div><h1>Trade Calculator</h1><p>TraderOS-style position sizing, risk-to-levels, trade analysis and target-price workflows.</p></div><span className="badge warn">Planning only · no execution</span></header>
  <div className="grid grid-dash calculator-layout">
   <section className="panel"><h2 className="panel-title">CALCULATION MODE</h2><div className="calculator-modes">{modes.map(m=><button type="button" key={m.id} className={mode===m.id?"calculator-mode active":"calculator-mode"} onClick={()=>setMode(m.id)}><strong>{m.name}</strong><small>{m.hint}</small></button>)}</div>
    <h2 className="panel-title" style={{marginTop:18}}>TRADE INPUTS</h2><div className="journal-form-grid">
     <label className="journal-field"><span>Instrument</span><select value={symbol} onChange={e=>selectInstrument(e.target.value)}>{presets.map(p=><option key={p.symbol} value={p.symbol}>{p.label}</option>)}</select></label>
     <label className="journal-field"><span>Direction</span><select value={direction} onChange={e=>setDirection(e.target.value as "long"|"short")}><option value="long">Buy / Long</option><option value="short">Sell / Short</option></select></label>
     <label className="journal-field"><span>Journal account</span><select value={account} onChange={e=>setAccount(e.target.value)}><option value="">Manual account</option>{accounts.data?.filter(a=>!a.archived).map(a=><option key={a.id} value={a.id}>{a.name} · {a.currency}</option>)}</select></label>
     <label className="journal-field"><span>Currency</span><select value={currency} onChange={e=>setCurrency(e.target.value)}>{["USD","EUR","GBP","NGN","CHF","JPY","USDT"].map(c=><option key={c}>{c}</option>)}</select></label>
     <label className="journal-field"><span>Balance / equity</span><input type="number" min="0" value={balance} onChange={e=>setBalance(e.target.value)}/></label>
     <label className="journal-field"><span>Entry price</span><input type="number" min="0" step="any" value={entry} onChange={e=>setEntry(e.target.value)}/></label>
     {needSL&&<label className="journal-field"><span>Stop-loss</span><input type="number" min="0" step="any" value={stop} onChange={e=>setStop(e.target.value)}/></label>}
     {needTP&&<label className="journal-field"><span>Take-profit</span><input type="number" min="0" step="any" value={target} onChange={e=>setTarget(e.target.value)}/></label>}
     {needQty&&<label className="journal-field"><span>Position size / quantity</span><input type="number" min="0" step="any" value={qty} onChange={e=>setQty(e.target.value)}/></label>}
     {needRiskPct&&<label className="journal-field"><span>Risk per trade (%)</span><input type="number" min="0" max="100" step="any" value={riskPct} onChange={e=>setRiskPct(e.target.value)}/></label>}
     {needRiskMoney&&<label className="journal-field"><span>Risk amount ({currency})</span><input type="number" min="0" step="any" value={risk} onChange={e=>setRisk(e.target.value)}/></label>}
     {needReward&&<label className="journal-field"><span>Target reward ({currency})</span><input type="number" min="0" step="any" value={reward} onChange={e=>setReward(e.target.value)}/></label>}
     <label className="journal-field"><span>Contract / multiplier</span><input type="number" min="0" step="any" value={contract} onChange={e=>setContract(e.target.value)}/></label>
     <label className="journal-field"><span>Volume step</span><input type="number" min="0" step="any" value={step} onChange={e=>setStep(e.target.value)}/></label>
     <label className="journal-field"><span>Pip / point size</span><input type="number" min="0" step="any" value={pip} onChange={e=>setPip(e.target.value)}/></label>
    </div><p className="muted" style={{fontSize:12,marginTop:12}}>Contract value and FX quote-to-account conversion are broker-dependent. Verify instrument contract/tick values, especially non-USD FX, gold, CFDs and crypto. Results are estimates; no order is placed.</p>
   </section>
   <section className="panel"><h2 className="panel-title">CALCULATION RESULTS</h2>{result&&"error" in result?<p className="error" role="alert">{result.error}</p>:result?<>
    {"budget" in result&&<div className="metric"><span className="metric-label">Risk budget</span><strong className="metric-value">{fmtMoney(result.budget,currency)}</strong></div>}
    {"size" in result&&<div className="metric" style={{marginTop:12}}><span className="metric-label">Position size</span><strong className="metric-value pos">{Number(result.size).toLocaleString(undefined,{maximumFractionDigits:8})}</strong></div>}
    <div className="journal-result-grid" style={{marginTop:14}}>
     {"dist" in result&&<div><span className="metric-label">Price distance</span><strong className="metric-value">{Number(result.dist).toLocaleString(undefined,{maximumFractionDigits:8})}</strong></div>}
     {"pips" in result&&<div><span className="metric-label">Pips / points</span><strong className="metric-value">{Number(result.pips).toFixed(2)}</strong></div>}
     {"raw" in result&&<div><span className="metric-label">Unrounded size</span><strong className="metric-value">{Number(result.raw).toLocaleString(undefined,{maximumFractionDigits:8})}</strong></div>}
     {"risk" in result&&<div><span className="metric-label">Risk</span><strong className="metric-value neg">{fmtMoney(result.risk,currency)}</strong></div>}
     {"riskPct" in result&&<div><span className="metric-label">Risk percent</span><strong className="metric-value">{Number(result.riskPct).toFixed(3)}%</strong></div>}
     {"reward" in result&&<div><span className="metric-label">Potential reward</span><strong className="metric-value pos">{fmtMoney(result.reward,currency)}</strong></div>}
     {"rr" in result&&<div><span className="metric-label">Risk / reward</span><strong className="metric-value">1 : {Number(result.rr).toFixed(2)}</strong></div>}
     {"stopPrice" in result&&<div><span className="metric-label">Calculated stop</span><strong className="metric-value">{result.stopPrice.toLocaleString(undefined,{maximumFractionDigits:8})}</strong></div>}
     {"targetPrice" in result&&<div><span className="metric-label">Calculated target</span><strong className="metric-value">{result.targetPrice.toLocaleString(undefined,{maximumFractionDigits:8})}</strong></div>}
    </div><Link className="btn primary" style={{display:"inline-flex",marginTop:18}} href={journalLink}>Add plan to Journal</Link>
   </>:<p className="muted">Enter your trade inputs to calculate.</p>}</section>
  </div>
 </div>;
}
