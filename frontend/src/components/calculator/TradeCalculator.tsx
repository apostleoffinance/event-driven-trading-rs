"use client";
import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { journalApi } from "@/lib/api/journal";
import { marketDataApi } from "@/lib/api/market-data";
import { hasSession } from "@/lib/auth/session";

type Mode = "fixed_risk_sl" | "risk_to_levels" | "entry_sl_to_size" | "trade_analysis" | "target_distance";
const presets = [
 {symbol:"EURUSD", label:"EURUSD · FX", contract:100000, step:0.01, pip:0.0001, quote:"USD", entry:"1.08500", sl:"1.08000"},
 {symbol:"GBPUSD", label:"GBPUSD · FX", contract:100000, step:0.01, pip:0.0001, quote:"USD", entry:"1.27000", sl:"1.26500"},
 {symbol:"USDJPY", label:"USDJPY · FX (JPY quote)", contract:100000, step:0.01, pip:0.01, quote:"JPY", entry:"155.000", sl:"154.500"},
 {symbol:"XAUUSD", label:"XAUUSD · Gold", contract:100, step:0.01, pip:0.01, quote:"USD", entry:"2700", sl:"2695"},
 {symbol:"BTCUSD", label:"BTCUSD · Crypto", contract:1, step:0.001, pip:1, quote:"USD", entry:"80000", sl:"79000"},
 {symbol:"US100", label:"US100 · Index CFD", contract:1, step:1, pip:1, quote:"USD", entry:"20000", sl:"19950"},
 {symbol:"CUSTOM", label:"Custom broker instrument", contract:1, step:0.01, pip:0.01, quote:"USD", entry:"100", sl:"99"},
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
 const [previewTab,setPreviewTab]=useState<"setup"|"chart"|"analysis">("setup"),[chartTimeframe,setChartTimeframe]=useState("M15"),[copiedSetup,setCopiedSetup]=useState(false);
 const [account,setAccount]=useState(""),[currency,setCurrency]=useState("USD"),[balance,setBalance]=useState("10000"),[riskPct,setRiskPct]=useState("1");
 const [quoteCurrency,setQuoteCurrency]=useState("USD"),[allowStaleFx,setAllowStaleFx]=useState(false);
 const [policyAssessmentInput,setPolicyAssessmentInput]=useState<null|{currency:string;risk_amount:number;potential_reward:number|null;equity:number;peak_equity?:number;start_of_day_balance?:number;start_of_day_equity?:number;current_open_notional?:number;lot_size:number;trade_notional:number}>(null);
 const [assessmentRun,setAssessmentRun]=useState(0),[assessmentFingerprint,setAssessmentFingerprint]=useState("");
 const [peakEquity,setPeakEquity]=useState(""),[startDayBalance,setStartDayBalance]=useState(""),[startDayEquity,setStartDayEquity]=useState(""),[currentOpenNotional,setCurrentOpenNotional]=useState("");
 const [entry,setEntry]=useState("1.08500"),[stop,setStop]=useState("1.08000"),[target,setTarget]=useState("1.09500"),[qty,setQty]=useState("0.20");
 const [risk,setRisk]=useState("100"),[reward,setReward]=useState("200"),[contract,setContract]=useState("100000"),[step,setStep]=useState("0.01"),[pip,setPip]=useState("0.0001");
 useEffect(()=>{const a=accounts.data?.find(x=>x.id===account);if(a){setCurrency(a.currency.toUpperCase());setBalance(a.initial_balance);setPeakEquity("");setStartDayBalance("");setStartDayEquity("");setCurrentOpenNotional("");setPolicyAssessmentInput(null);setAssessmentFingerprint("");}},[account,accounts.data]);
 const riskPolicy=useQuery({queryKey:["journal","risk-policy",account],queryFn:()=>journalApi.getRiskPolicy(account),enabled:auth&&Boolean(account)});
 const policyRiskPct=riskPolicy.data?.policy.risk_per_trade_pct;
 useEffect(()=>{if(policyRiskPct!==undefined){setRiskPct(String(policyRiskPct));}},[account,policyRiskPct,riskPolicy.data?.version]);
 const conversion=useQuery({queryKey:["fx-conversion",quoteCurrency,currency,allowStaleFx],queryFn:()=>marketDataApi.fxConvert({from:quoteCurrency,to:currency,amount:1,allow_stale:allowStaleFx}),enabled:quoteCurrency!==currency,staleTime:30000,refetchInterval:30000,refetchIntervalInBackground:false,retry:1});
 const quoteToAccountRate=quoteCurrency===currency?1:(conversion.data?Number(conversion.data.rate):null);
 const conversionFresh=quoteCurrency===currency||Boolean(conversion.data&&(conversion.data.freshness==="fresh"||allowStaleFx));
 const currentAssessmentFingerprint=JSON.stringify([account,mode,entry,stop,target,qty,balance,riskPct,risk,reward,contract,step,pip,direction,peakEquity,startDayBalance,startDayEquity,currentOpenNotional,quoteCurrency,currency,quoteToAccountRate,conversion.data?.freshness,conversion.data?.timestamp,allowStaleFx,riskPolicy.data?.version]);
 const policyAssessment=useQuery({queryKey:["journal","risk-policy-assessment",account,assessmentRun,policyAssessmentInput],queryFn:()=>journalApi.evaluateRiskPolicy(account,policyAssessmentInput!),enabled:Boolean(account&&policyAssessmentInput)});
 const selectInstrument=(s:string)=>{const p=presets.find(x=>x.symbol===s)!;setSymbol(s);setQuoteCurrency(p.quote);setContract(String(p.contract));setStep(String(p.step));setPip(String(p.pip));setEntry(p.entry);setStop(p.sl);setPolicyAssessmentInput(null);};
 const result=useMemo(()=>{
  const e=num(entry),sl=num(stop),tp=num(target),q=num(qty),b=num(balance),rp=num(riskPct),r=num(risk),rw=num(reward),cv=num(contract),st=num(step),ps=num(pip),fx=quoteToAccountRate??0;
  if(![e,b,cv,st,ps].every(Number.isFinite)||e<=0||b<=0||cv<=0||st<=0||ps<=0)return {error:"Entry, balance, contract size, quantity step and pip size must be positive."};
  if(fx<=0||!conversionFresh)return {error:quoteCurrency+" → "+currency+" live conversion is required before calculating. "+(allowStaleFx?"The stale quote could not be fetched.":"Use the explicit stale-rate option only if you accept stale conversion data.")};
  if(mode==="fixed_risk_sl"||mode==="entry_sl_to_size"){
   if(!Number.isFinite(sl)||sl<=0||(direction==="long"?sl>=e:sl<=e))return {error:"Stop-loss must be below entry for Long and above entry for Short."};
   if(rp<=0||rp>100)return {error:"Risk percentage must be greater than 0 and no more than 100."};
   const budget=b*rp/100,dist=Math.abs(e-sl),raw=budget/(dist*cv*fx),size=roundDown(raw,st),actualRisk=size*dist*cv*fx;
   if(size<=0)return {error:"Risk budget is below minimum volume step. Increase risk or reduce step."};
   const targetDist=Number.isFinite(tp)&&tp>0?Math.abs(tp-e):0, potentialReward=size*targetDist*cv*fx;
   return {size,budget,dist,pips:dist/ps,raw,risk:actualRisk,riskPct:actualRisk/b*100,reward:potentialReward,rr:actualRisk>0?potentialReward/actualRisk:0,stopPrice:sl,targetPrice:tp};
  }
  if(mode==="risk_to_levels"){
   if(!Number.isFinite(q)||q<=0||r<=0||rw<=0)return {error:"Quantity, risk amount and target reward must be positive."};
   const slDist=r/(q*cv*fx),tpDist=rw/(q*cv*fx);
   return {size:q,budget:r,dist:slDist,pips:slDist/ps,risk:r,reward:rw,rr:rw/r,stopPrice:e+(direction==="long"?-1:1)*slDist,targetPrice:e+(direction==="long"?1:-1)*tpDist};
  }
  if(!Number.isFinite(q)||q<=0)return {error:"Position quantity must be positive."};
  if(mode==="target_distance"){
   if(rw<=0)return {error:"Target reward must be positive."};
   const dist=rw/(q*cv*fx);return {size:q,dist,pips:dist/ps,targetPrice:e+(direction==="long"?1:-1)*dist,reward:rw};
  }
  if(!Number.isFinite(sl)||!Number.isFinite(tp)||sl<=0||tp<=0||(direction==="long"?(sl>=e||tp<=e):(sl<=e||tp>=e)))return {error:"For Long, SL must be below entry and TP above. For Short, reverse these levels."};
  const dist=Math.abs(e-sl),targetDist=Math.abs(tp-e),estimatedRisk=q*dist*cv*fx,potentialReward=q*targetDist*cv*fx;
  return {size:q,dist,pips:dist/ps,risk:estimatedRisk,reward:potentialReward,riskPct:estimatedRisk/b*100,rr:potentialReward/estimatedRisk};
 },[mode,entry,stop,target,qty,balance,riskPct,risk,reward,contract,step,pip,direction,quoteToAccountRate,conversionFresh,quoteCurrency,currency,allowStaleFx]);
 const stopValue=result&&!('error' in result)&&('stopPrice' in result)?result.stopPrice:stop;
 const targetValue=result&&!('error' in result)&&('targetPrice' in result)?result.targetPrice:target;
 const journalLink=result&&!('error' in result)&&('size' in result)?"/journal?newTrade=1&symbol="+encodeURIComponent(symbol)+"&direction="+direction+"&entry="+encodeURIComponent(entry)+"&stop="+encodeURIComponent(String(stopValue))+"&quantity="+encodeURIComponent(String(result.size))+"&risk="+encodeURIComponent(String(("risk" in result ? result.risk : "")))+"&take_profit="+encodeURIComponent(String(targetValue))+"&account="+encodeURIComponent(account)+"&currency="+encodeURIComponent(currency):"/journal";
 const chartCandles=useQuery({queryKey:["calculator-preview-candles",symbol,chartTimeframe],queryFn:()=>marketDataApi.candles({instrument:symbol,timeframe:chartTimeframe,limit:64}),enabled:previewTab==="chart",staleTime:15000,retry:1});
 const previewEntry=num(entry);
 const previewStop=result&&!('error' in result)&&('stopPrice' in result)?Number(result.stopPrice):num(stop);
 const previewTarget=result&&!('error' in result)&&('targetPrice' in result)?Number(result.targetPrice):num(target);
 const previewSize=result&&!('error' in result)&&('size' in result)?Number(result.size):num(qty);
 const previewRisk=result&&!('error' in result)&&('risk' in result)?Number(result.risk):result&&!('error' in result)&&('budget' in result)?Number(result.budget):null;
 const previewReward=result&&!('error' in result)&&('reward' in result)?Number(result.reward):null;
 const previewRR=result&&!('error' in result)&&('rr' in result)?Number(result.rr):previewRisk&&previewReward?previewReward/previewRisk:null;
 const previewRiskPips=Number.isFinite(previewEntry)&&Number.isFinite(previewStop)&&num(pip)>0?Math.abs(previewEntry-previewStop)/num(pip):null;
 const previewRewardPips=Number.isFinite(previewEntry)&&Number.isFinite(previewTarget)&&num(pip)>0?Math.abs(previewEntry-previewTarget)/num(pip):null);
 const previewLevelsValid=Number.isFinite(previewEntry)&&previewEntry>0&&Number.isFinite(previewStop)&&Number.isFinite(previewTarget)&&previewStop>0&&previewTarget>0&&(direction==="long"?previewStop<previewEntry&&previewTarget>previewEntry:previewStop>previewEntry&&previewTarget<previewEntry);
 const previewPricePrecision=Math.max(2,Math.min(8,(entry.split(".")[1]||"").length,(pip.split(".")[1]||"").length+1));
 const previewPrice=(v:number)=>Number.isFinite(v)?v.toLocaleString(undefined,{minimumFractionDigits:previewPricePrecision,maximumFractionDigits:previewPricePrecision}):"—";
 const previewChartData=chartCandles.data?.candles??[];
 const previewChartPrices=previewLevelsValid?[...previewChartData.flatMap(c=>[c.low,c.high]),previewEntry,previewStop,previewTarget]:previewChartData.flatMap(c=>[c.low,c.high]);
 const previewChartMin=previewChartPrices.length?Math.min(...previewChartPrices):0;
 const previewChartMax=previewChartPrices.length?Math.max(...previewChartPrices):1;
 const previewChartRange=previewChartMax-previewChartMin||1;
 const previewChartY=(price:number)=>176-((price-previewChartMin)/previewChartRange)*152;
 const previewZoneScale=120/Math.max(1,previewRiskPips??0,previewRewardPips??0);
 const previewProfitZoneHeight=Math.max(44,Math.min(140,(previewRewardPips??0)*previewZoneScale));
 const previewLossZoneHeight=Math.max(44,Math.min(140,(previewRiskPips??0)*previewZoneScale));
 const previewChartX=(index:number)=>12+index*(576/Math.max(1,previewChartData.length-1));
 const copyTradeSetup=async()=>{const text=[`EdgeNest Trade Plan · ${symbol} · ${direction.toUpperCase()}`,`Entry: ${previewPrice(previewEntry)}`,`Stop Loss: ${previewPrice(previewStop)}`,`Take Profit: ${previewPrice(previewTarget)}`,`Size: ${previewSize}`,`Risk: ${previewRisk==null?"—":fmtMoney(previewRisk,currency)}`,`Reward: ${previewReward==null?"—":fmtMoney(previewReward,currency)}`,`Risk/Reward: ${previewRR==null?"—":"1 : "+previewRR.toFixed(2)}`].join("\ const previewRiskShare=previewRisk!==null&&previewReward!==null&&previewRisk+previewReward>0?previewRisk/(previewRisk+previewReward):1/3;\nn");try{await navigator.clipboard.writeText(text);setCopiedSetup(true);window.setTimeout(()=>setCopiedSetup(false),1800);}catch{setCopiedSetup(false);}};
 const needQty=mode==="risk_to_levels"||mode==="trade_analysis"||mode==="target_distance";
 const needSL=mode!=="risk_to_levels"&&mode!=="target_distance";
 const needTP=mode==="trade_analysis"||mode==="fixed_risk_sl"||mode==="entry_sl_to_size";
 const needRiskPct=mode==="fixed_risk_sl"||mode==="entry_sl_to_size";
 const needRiskMoney=mode==="risk_to_levels";
 const needReward=mode==="risk_to_levels"||mode==="target_distance";
 return <div>
  <header className="page-header"><div><h1>Trade Calculator</h1><p>TraderOS-style position sizing with backend FX conversion and account risk policies.</p></div><div style={{display:"flex",gap:8,alignItems:"center"}}><Link className="btn" href="/risk/policies">Risk Policy</Link><span className="badge warn">Planning only · no execution</span></div></header>
  <div className="grid grid-dash calculator-layout">
   <section className="panel"><h2 className="panel-title">CALCULATION MODE</h2><div className="calculator-modes">{modes.map(m=><button type="button" key={m.id} className={mode===m.id?"calculator-mode active":"calculator-mode"} onClick={()=>setMode(m.id)}><strong>{m.name}</strong><small>{m.hint}</small></button>)}</div>
    <h2 className="panel-title" style={{marginTop:18}}>TRADE INPUTS</h2><div className="journal-form-grid">
     <label className="journal-field"><span>Instrument</span><select value={symbol} onChange={e=>selectInstrument(e.target.value)}>{presets.map(p=><option key={p.symbol} value={p.symbol}>{p.label}</option>)}</select></label>
     <label className="journal-field"><span>Direction</span><select value={direction} onChange={e=>setDirection(e.target.value as "long"|"short")}><option value="long">Buy / Long</option><option value="short">Sell / Short</option></select></label>
     <label className="journal-field"><span>Journal account</span><select value={account} onChange={e=>setAccount(e.target.value)}><option value="">Manual account</option>{accounts.data?.filter(a=>!a.archived).map(a=><option key={a.id} value={a.id}>{a.name} · {a.currency}</option>)}</select></label>
     <label className="journal-field"><span>Account currency</span><select value={currency} onChange={e=>setCurrency(e.target.value)} disabled={Boolean(account)}>{Array.from(new Set([currency,"USD","EUR","GBP","JPY","CHF","CAD","AUD","NZD","NGN","TRY","ZAR","MXN","SEK","SGD","HKD","USDT"])).map(c=><option key={c}>{c}</option>)}</select></label>
     {symbol==="CUSTOM"&&<label className="journal-field"><span>Instrument quote currency</span><select value={quoteCurrency} onChange={e=>setQuoteCurrency(e.target.value)}>{Array.from(new Set([currency,"USD","EUR","GBP","JPY","CHF","CAD","AUD","NZD","NGN","TRY","ZAR","MXN","SEK","SGD","HKD","USDT"])).map(c=><option key={c}>{c}</option>)}</select></label>}
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
     <label className="journal-field"><span>FX conversion</span><div className="metric-sub">{quoteCurrency} → {currency}: {quoteCurrency===currency?"1 (same currency)":conversion.data?Number(conversion.data.rate).toPrecision(8):conversion.isLoading?"Fetching live rate…":"Unavailable"}</div></label>
    </div><div style={{marginTop:10}}><label style={{display:"flex",gap:8,alignItems:"center"}}><input type="checkbox" checked={allowStaleFx} onChange={e=>{setAllowStaleFx(e.target.checked);setPolicyAssessmentInput(null);}}/>Allow stale FX quote for an explicitly marked estimate</label>{conversion.data?<p className="muted" style={{marginTop:6}}>Rate source: {conversion.data.source} · {conversion.data.method} · {conversion.data.freshness} · age {conversion.data.age_seconds??"—"}s · {conversion.data.timestamp??"same currency"}</p>:null}{conversion.isError&&quoteCurrency!==currency?<p className="error" role="alert">FX conversion unavailable: {conversion.error instanceof Error?conversion.error.message:"Provider rate unavailable"}</p>:null}</div><p className="muted" style={{fontSize:12,marginTop:12}}>FX conversion is obtained from the backend live market-data service. Broker contract size, tick value, commissions, swaps, and slippage are not inferred. Stale quotes require explicit acceptance.</p>
   </section>
   <section className="panel calculator-preview-panel">
    <div className="trade-preview-header"><div><span className="trade-preview-eyebrow">TRADE SETUP PREVIEW</span><h2>{symbol} <span className={direction==="long"?"trade-direction-long":"trade-direction-short"}>{direction==="long"?"BUY / LONG":"SELL / SHORT"}</span></h2><p className="muted">Live preview of your calculator inputs</p></div><span className="trade-preview-rr">{previewLevelsValid&&previewRR!==null&&Number.isFinite(previewRR)?`1 : ${previewRR.toFixed(2)}`:"R/R —"}</span></div>
    <div className="trade-preview-tabs" role="tablist" aria-label="Trade preview modes">
     {([{id:"setup",label:"Setup",icon:"◉"},{id:"chart",label:"Chart",icon:"⌁"},{id:"analysis",label:"Analysis",icon:"▤"}] as const).map(t=><button key={t.id} type="button" role="tab" aria-selected={previewTab===t.id} className={previewTab===t.id?"trade-preview-tab active":"trade-preview-tab"} onClick={()=>setPreviewTab(t.id)}><span aria-hidden="true">{t.icon}</span>{t.label}</button>)}
    </div>
    {result&&"error" in result?<div className="trade-preview-error" role="alert"><strong>Setup needs attention</strong><p>{result.error}</p></div>:previewTab==="setup"?<>
     <div className="trade-level-visual" aria-label="Trade risk and reward levels">
      <div className="trade-level-row trade-level-tp"><div><span className="trade-level-tag">TAKE PROFIT</span><strong>{previewLevelsValid?previewPrice(previewTarget):"—"}</strong><small>{previewRewardPips!==null&&Number.isFinite(previewRewardPips)?`+${previewRewardPips.toFixed(1)} pips`:"Target not set"}</small></div><div className="trade-level-value"><strong>{previewReward===null?"—":`+${fmtMoney(previewReward,currency)}`}</strong><small>Potential reward</small></div></div>
      <div className="trade-profit-zone" style={{height:previewProfitZoneHeight}}><div className="trade-zone-caption"><span>PROFIT ZONE</span><strong>{previewReward===null?"Reward unavailable":`+${fmtMoney(previewReward,currency)}`}</strong></div><div className="trade-zone-track"><span/></div><small>{previewRewardPips!==null&&Number.isFinite(previewRewardPips)?`Target distance · ${previewRewardPips.toFixed(1)} pips`:"Set a valid target to visualize reward"}</small></div>
      <div className="trade-level-row trade-level-entry"><div><span className="trade-level-tag">ENTRY PRICE</span><strong>{previewPrice(previewEntry)}</strong><small>{direction==="long"?"BUY":"SELL"} · {Number.isFinite(previewSize)?previewSize.toLocaleString(undefined,{maximumFractionDigits:8}):"—"} lots / units</small></div><div className="trade-level-value"><strong>Entry</strong><small>Reference price</small></div></div>
      <div className="trade-loss-zone" style={{height:previewLossZoneHeight}}><div className="trade-zone-caption"><span>STOP LOSS ZONE</span><strong>{previewRisk===null?"Risk unavailable":`−${fmtMoney(previewRisk,currency)}`}</strong></div><div className="trade-zone-track"><span/></div><small>{previewRiskPips!==null&&Number.isFinite(previewRiskPips)?`Stop distance · ${previewRiskPips.toFixed(1)} pips`:"Set a valid stop to visualize risk"}</small></div>
      <div className="trade-level-row trade-level-sl"><div><span className="trade-level-tag">STOP LOSS</span><strong>{previewLevelsValid?previewPrice(previewStop):"—"}</strong><small>Defined risk level</small></div><div className="trade-level-value"><strong>{previewRisk===null?"—":`−${fmtMoney(previewRisk,currency)}`}</strong><small>Potential loss</small></div></div>
     </div>
     <div className="trade-preview-summary"><div className="trade-preview-stat"><span>Risk budget</span><strong className="trade-negative">{previewRisk===null?"—":fmtMoney(previewRisk,currency)}</strong></div><div className="trade-preview-stat"><span>Potential reward</span><strong className="trade-positive">{previewReward===null?"—":fmtMoney(previewReward,currency)}</strong></div><div className="trade-preview-stat"><span>Position size</span><strong>{Number.isFinite(previewSize)?previewSize.toLocaleString(undefined,{maximumFractionDigits:8}):"—"}</strong></div><div className="trade-preview-stat"><span>Stop distance</span><strong>{previewRiskPips!==null&&Number.isFinite(previewRiskPips)?previewRiskPips.toFixed(1)+" pips":"—"}</strong></div></div>
     <div className="trade-preview-riskbar"><div><span>Risk</span><strong>{previewRisk===null?"—":fmtMoney(previewRisk,currency)}</strong></div><div className="trade-risk-reward-bar"><span style={{width:`${previewRiskShare*100}%`}}/><span style={{width:`${(1-previewRiskShare)*100}%`}}/></div><div><span>Reward</span><strong>{previewReward===null?"—":fmtMoney(previewReward,currency)}</strong></div></div>
    </>:previewTab==="chart"?<div className="trade-mini-chart">
      <div className="trade-mini-chart-toolbar"><div><strong>{symbol}</strong><span className="muted"> · {chartTimeframe}</span></div><select aria-label="Chart timeframe" value={chartTimeframe} onChange={e=>setChartTimeframe(e.target.value)}>{["M1","M5","M15","M30","H1","H4","D1"].map(tf=><option key={tf}>{tf}</option>)}</select></div>
      {chartCandles.isLoading?<div className="trade-chart-empty">Loading real market candles…</div>:chartCandles.isError?<div className="trade-chart-empty"><strong>Market data unavailable</strong><p>Could not load candles for {symbol}. Check the configured market-data provider.</p><button className="btn" onClick={()=>void chartCandles.refetch()}>Retry</button></div>:previewChartData.length===0?<div className="trade-chart-empty"><strong>No historical candles available</strong><p>Connect or configure a market-data provider to display this chart. No sample candles are shown.</p></div>:<div className="trade-mini-chart-svg-wrap"><svg viewBox="0 0 600 200" role="img" aria-label={`${symbol} historical candlestick chart with trade levels`} className="trade-mini-chart-svg">
       {[0,1,2,3].map(i=><line key={i} x1="8" x2="592" y1={18+i*48} y2={18+i*48} className="trade-chart-gridline"/> )}
       {previewChartData.map((c,i)=>{const x=previewChartX(i),up=c.close>=c.open,y1=previewChartY(Math.max(c.open,c.close)),y2=previewChartY(Math.min(c.open,c.close)),bodyH=Math.max(2,y2-y1);return <g key={c.timestamp_ms}><line x1={x} x2={x} y1={previewChartY(c.high)} y2={previewChartY(c.low)} className={up?"trade-candle-up":"trade-candle-down"}/><rect x={x-2.5} y={y1} width="5" height={bodyH} className={up?"trade-candle-up-fill":"trade-candle-down-fill"}/></g>;})}
       {previewLevelsValid&&[["TP",previewTarget,"trade-chart-tp"],["ENTRY",previewEntry,"trade-chart-entry"],["SL",previewStop,"trade-chart-sl"]].map(([label,value,cls])=><g key={label as string}><line x1="8" x2="592" y1={previewChartY(Number(value))} y2={previewChartY(Number(value))} className={cls as string}/><text x="12" y={Math.max(12,previewChartY(Number(value))-4)} className="trade-chart-level-label">{label as string} {previewPrice(Number(value))}</text></g>)}
      </svg><div className="trade-chart-footnote">Candles from configured EdgeNest market-data provider · Historical view only</div></div>}
     </div>:<div className="trade-analysis-view">
      <div className="trade-analysis-grid"><div><span>Account equity</span><strong>{fmtMoney(num(balance),currency)}</strong></div><div><span>Risk % of equity</span><strong>{previewRisk!==null&&num(balance)>0?(previewRisk/num(balance)*100).toFixed(3)+"%":"—"}</strong></div><div><span>Risk / Reward</span><strong>{previewRR!==null&&Number.isFinite(previewRR)?`1 : ${previewRR.toFixed(2)}`:"—"}</strong></div><div><span>Position size</span><strong>{Number.isFinite(previewSize)?previewSize.toLocaleString(undefined,{maximumFractionDigits:8}):"—"}</strong></div><div><span>Stop distance</span><strong>{previewRiskPips!==null&&Number.isFinite(previewRiskPips)?previewRiskPips.toFixed(2)+" pips":"—"}</strong></div><div><span>Target distance</span><strong>{previewRewardPips!==null&&Number.isFinite(previewRewardPips)?previewRewardPips.toFixed(2)+" pips":"—"}</strong></div></div>
      <div className="trade-analysis-note"><strong>Risk is a planned estimate, not guaranteed loss.</strong><p>Actual results may differ due to spread, commissions, swaps, slippage, gaps and broker execution. This calculator does not submit orders.</p></div>
      {account&&"risk" in result&&Number(result.risk)>0?<div className="panel trade-policy-panel"><h3 className="panel-title">ACCOUNT POLICY CHECK</h3>{riskPolicy.isLoading?<p className="muted">Loading risk policy…</p>:riskPolicy.isError?<p className="error">Account policy unavailable. Configure the account risk policy first.</p>:<><p className="muted">{riskPolicy.data?.policy.profile_name} · {riskPolicy.data?.policy.risk_per_trade_pct}% risk/trade · {riskPolicy.data?.policy.firm_name||"Personal / custom account"} {riskPolicy.data?.version===0?"(default profile, not saved)":""}</p><div className="journal-form-grid" style={{marginBottom:10}}><label className="journal-field"><span>Peak equity for trailing DD</span><input type="number" min="0.01" step="any" value={peakEquity} onChange={e=>setPeakEquity(e.target.value)} placeholder="Needed for trailing DD"/></label><label className="journal-field"><span>Start-of-day balance</span><input type="number" min="0.01" step="any" value={startDayBalance} onChange={e=>setStartDayBalance(e.target.value)} placeholder="Needed for SOD balance rule"/></label><label className="journal-field"><span>Start-of-day equity</span><input type="number" min="0.01" step="any" value={startDayEquity} onChange={e=>setStartDayEquity(e.target.value)} placeholder="Needed for SOD equity rule"/></label><label className="journal-field"><span>Current open notional ({currency})</span><input type="number" min="0" step="any" value={currentOpenNotional} onChange={e=>setCurrentOpenNotional(e.target.value)} placeholder="Needed for leverage rule"/></label></div><button className="btn" type="button" onClick={()=>{const e=num(entry),cv=num(contract),fx=quoteToAccountRate??0;setPolicyAssessmentInput({currency:currency,risk_amount:Number(result.risk),potential_reward:("reward" in result)?Number(result.reward):null,equity:num(balance),...(peakEquity.trim()?{peak_equity:num(peakEquity)}:{}),...(startDayBalance.trim()?{start_of_day_balance:num(startDayBalance)}:{}),...(startDayEquity.trim()?{start_of_day_equity:num(startDayEquity)}:{}),...(currentOpenNotional.trim()?{current_open_notional:num(currentOpenNotional)}:{}),lot_size:Number(("size" in result)?result.size:0),trade_notional:Number(("size" in result)?result.size:0)*e*cv*fx});setAssessmentFingerprint(currentAssessmentFingerprint);setAssessmentRun(v=>v+1);}} disabled={!conversionFresh||quoteToAccountRate==null}>Assess against saved account policy</button>{policyAssessmentInput&&policyAssessment.isLoading?<p className="muted">Assessing policy and daily account history…</p>:null}{policyAssessment.data&&assessmentFingerprint===currentAssessmentFingerprint?<div style={{marginTop:10}}><strong className={String(policyAssessment.data.status)==="blocked"?"neg":String(policyAssessment.data.status)==="warning"?"warn":"pos"}>{String(policyAssessment.data.status).toUpperCase()}</strong><p className="muted">Daily P&amp;L: {String(policyAssessment.data.daily_pnl)} {currency} · Trades today: {String(policyAssessment.data.trades_today)} · Open positions: {String(policyAssessment.data.open_positions)}</p>{Array.isArray(policyAssessment.data.violations)&&policyAssessment.data.violations.map((v,i)=><p key={i} className="muted">{String((v as Record<string,unknown>).severity).toUpperCase()}: {String((v as Record<string,unknown>).message)}</p>)}</div>:policyAssessment.data?<p className="muted">This assessment is stale because calculator inputs or account policy changed. Re-run the account policy check.</p>:null}</>}</div>:null}
     </div>}
    <div className="trade-preview-actions"><button className="btn" type="button" onClick={()=>void copyTradeSetup()}>⧉ {copiedSetup?"Copied!":"Copy trade setup"}</button><Link className="btn primary" href={journalLink}>Add plan to Journal →</Link></div>
    <p className="trade-preview-disclaimer">Planning tool only · Potential outcomes are estimates before fees and slippage · No order execution</p>
   </section>
  </div>
 </div>;
}
