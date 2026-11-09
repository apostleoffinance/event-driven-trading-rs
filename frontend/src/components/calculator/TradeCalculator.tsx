"use client";
import Link from "next/link";
import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { journalApi } from "@/lib/api/journal";
import { marketDataApi } from "@/lib/api/market-data";
import { hasSession } from "@/lib/auth/session";
import { TradeSetupPreview } from "@/components/calculator/TradeSetupPreview";

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
   const targetValid=Number.isFinite(tp)&&tp>0&&(direction==="long"?tp>e:tp<e);
   const targetDist=targetValid?Math.abs(tp-e):0;
   const potentialReward=targetDist>0?size*targetDist*cv*fx:null;
   return {size,budget,dist,pips:dist/ps,raw,risk:actualRisk,riskPct:actualRisk/b*100,targetPrice:targetValid?tp:null,targetDist,targetPips:targetDist/ps,reward:potentialReward,rr:potentialReward!==null&&actualRisk>0?potentialReward/actualRisk:null};
  }
  if(mode==="risk_to_levels"){
   if(!Number.isFinite(q)||q<=0||r<=0||rw<=0)return {error:"Quantity, risk amount and target reward must be positive."};
   const slDist=r/(q*cv*fx),tpDist=rw/(q*cv*fx);
   return {size:q,budget:r,dist:slDist,pips:slDist/ps,risk:r,riskPct:r/b*100,reward:rw,rr:rw/r,stopPrice:e+(direction==="long"?-1:1)*slDist,targetPrice:e+(direction==="long"?1:-1)*tpDist,targetDist:tpDist,targetPips:tpDist/ps};
  }
  if(!Number.isFinite(q)||q<=0)return {error:"Position quantity must be positive."};
  if(mode==="target_distance"){
   if(rw<=0)return {error:"Target reward must be positive."};
   const dist=rw/(q*cv*fx),stopValid=Number.isFinite(sl)&&sl>0&&(direction==="long"?sl<e:sl>e),stopDist=stopValid?Math.abs(e-sl):0,estimatedRisk=stopValid?q*stopDist*cv*fx:null;
   return {size:q,dist,pips:dist/ps,targetPrice:e+(direction==="long"?1:-1)*dist,targetDist:dist,targetPips:dist/ps,reward:rw,stopPrice:stopValid?sl:null,stopDist,stopPips:stopDist/ps,risk:estimatedRisk,riskPct:estimatedRisk!==null&&b>0?estimatedRisk/b*100:null,rr:estimatedRisk!==null&&estimatedRisk>0?rw/estimatedRisk:null};
  }
  if(!Number.isFinite(sl)||!Number.isFinite(tp)||sl<=0||tp<=0||(direction==="long"?(sl>=e||tp<=e):(sl<=e||tp>=e)))return {error:"For Long, SL must be below entry and TP above. For Short, reverse these levels."};
  const dist=Math.abs(e-sl),targetDist=Math.abs(tp-e),estimatedRisk=q*dist*cv*fx,potentialReward=q*targetDist*cv*fx;
  return {size:q,dist,pips:dist/ps,stopPips:dist,targetDist:targetDist,targetPips:targetDist/ps,risk:estimatedRisk,reward:potentialReward,riskPct:estimatedRisk/b*100,rr:potentialReward/estimatedRisk};
 },[mode,entry,stop,target,qty,balance,riskPct,risk,reward,contract,step,pip,direction,quoteToAccountRate,conversionFresh,quoteCurrency,currency,allowStaleFx]);
 const stopValue=result&&!('error' in result)&&('stopPrice' in result)&&typeof result.stopPrice==="number"?result.stopPrice:num(stop);
 const targetValue=result&&!('error' in result)&&('targetPrice' in result)&&typeof result.targetPrice==="number"?result.targetPrice:num(target);
 const previewData=useMemo(()=>{
  if(!result||"error" in result)return {valid:false,message:result&&"error" in result?result.error:"Enter your trade details to calculate.",size:num(qty),risk:null,reward:null,stopPips:null,targetPips:null,rr:null,riskPct:null,stop:num(stop),target:num(target)};
  const size="size" in result?Number(result.size):num(qty);
  const riskValue="risk" in result&&typeof result.risk==="number"?Number(result.risk):null;
  const rewardValue="reward" in result&&typeof result.reward==="number"?Number(result.reward):null;
  const stopPriceValue=Number(stopValue),targetPriceValue=Number(targetValue);
  const stopValid=Number.isFinite(stopPriceValue)&&stopPriceValue>0&&(direction==="long"?stopPriceValue<num(entry):stopPriceValue>num(entry));
  const targetValid=Number.isFinite(targetPriceValue)&&targetPriceValue>0&&(direction==="long"?targetPriceValue>num(entry):targetPriceValue<num(entry));
  const stopPipsValue="stopPips" in result&&typeof result.stopPips==="number"?Number(result.stopPips):("pips" in result?Number(result.pips):null);
  const targetPipsValue="targetPips" in result&&typeof result.targetPips==="number"?Number(result.targetPips):null;
  const rrValue="rr" in result&&typeof result.rr==="number"?Number(result.rr):null;
  const riskPctValue="riskPct" in result&&typeof result.riskPct==="number"?Number(result.riskPct):null;
  const valid=Number.isFinite(size)&&size>0&&stopValid&&targetValid&&riskValue!==null&&riskValue>0&&rewardValue!==null&&rewardValue>0;
  return {valid,message:!stopValid?"Set a valid Stop Loss on the correct side of entry.":!targetValid?"Set a valid Take Profit on the correct side of entry.":riskValue===null||rewardValue===null?"Complete the risk and reward inputs to finish the preview.":undefined,size,risk:riskValue,reward:rewardValue,stopPips:stopPipsValue,targetPips:targetPipsValue,rr:rrValue,riskPct:riskPctValue,stop:stopPriceValue,target:targetPriceValue};
 },[result,qty,stopValue,targetValue,direction,entry]);
 const journalLink=previewData.valid?"/journal?newTrade=1&symbol="+encodeURIComponent(symbol)+"&direction="+direction+"&entry="+encodeURIComponent(entry)+"&stop="+encodeURIComponent(String(stopValue))+"&quantity="+encodeURIComponent(String(previewData.size))+"&risk="+encodeURIComponent(String(previewData.risk))+"&take_profit="+encodeURIComponent(String(targetValue))+"&account="+encodeURIComponent(account)+"&currency="+encodeURIComponent(currency):"/journal";
 const needQty=mode==="risk_to_levels"||mode==="trade_analysis"||mode==="target_distance";
 const needSL=mode!=="risk_to_levels";
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
   <TradeSetupPreview
    symbol={symbol}
    direction={direction}
    currency={currency}
    entry={num(entry)}
    stop={Number(previewData.stop??stopValue)}
    target={Number(previewData.target??targetValue)}
    quantity={Number(previewData.size??num(qty))}
    risk={previewData.risk??null}
    reward={previewData.reward??null}
    riskPct={previewData.riskPct??null}
    stopPips={previewData.stopPips??null}
    targetPips={previewData.targetPips??null}
    rr={previewData.rr??null}
    balance={Number.isFinite(num(balance))&&num(balance)>0?num(balance):null}
    valid={previewData.valid}
    validationMessage={previewData.message}
    journalHref={journalLink}
   />
  </div>
  {account&&"risk" in result&&Number(result.risk)>0?<div className="panel" style={{marginTop:14}}><h3 className="panel-title">ACCOUNT POLICY CHECK</h3>{riskPolicy.isLoading?<p className="muted">Loading risk policy…</p>:riskPolicy.isError?<p className="error">Account policy unavailable. Configure the account risk policy first.</p>:<><p className="muted">{riskPolicy.data?.policy.profile_name} · {riskPolicy.data?.policy.risk_per_trade_pct}% risk/trade · {riskPolicy.data?.policy.firm_name||"Personal / custom account"} {riskPolicy.data?.version===0?"(default profile, not saved)":""}</p><div className="journal-form-grid" style={{marginBottom:10}}><label className="journal-field"><span>Peak equity for trailing DD</span><input type="number" min="0.01" step="any" value={peakEquity} onChange={e=>setPeakEquity(e.target.value)} placeholder="Needed for trailing DD"/></label><label className="journal-field"><span>Start-of-day balance</span><input type="number" min="0.01" step="any" value={startDayBalance} onChange={e=>setStartDayBalance(e.target.value)} placeholder="Needed for SOD balance rule"/></label><label className="journal-field"><span>Start-of-day equity</span><input type="number" min="0.01" step="any" value={startDayEquity} onChange={e=>setStartDayEquity(e.target.value)} placeholder="Needed for SOD equity rule"/></label><label className="journal-field"><span>Current open notional ({currency})</span><input type="number" min="0" step="any" value={currentOpenNotional} onChange={e=>setCurrentOpenNotional(e.target.value)} placeholder="Needed for leverage rule"/></label></div><button className="btn" type="button" onClick={()=>{const e=num(entry),cv=num(contract),fx=quoteToAccountRate??0;setPolicyAssessmentInput({currency:currency,risk_amount:Number(result.risk),potential_reward:("reward" in result)?Number(result.reward):null,equity:num(balance),...(peakEquity.trim()?{peak_equity:num(peakEquity)}:{}),...(startDayBalance.trim()?{start_of_day_balance:num(startDayBalance)}:{}),...(startDayEquity.trim()?{start_of_day_equity:num(startDayEquity)}:{}),...(currentOpenNotional.trim()?{current_open_notional:num(currentOpenNotional)}:{}),lot_size:Number(("size" in result)?result.size:0),trade_notional:Number(("size" in result)?result.size:0)*e*cv*fx});setAssessmentFingerprint(currentAssessmentFingerprint);setAssessmentRun(v=>v+1);}} disabled={!conversionFresh||quoteToAccountRate==null}>Assess against saved account policy</button>{policyAssessmentInput&&policyAssessment.isLoading?<p className="muted">Assessing policy and daily account history…</p>:null}{policyAssessment.data&&assessmentFingerprint===currentAssessmentFingerprint?<div style={{marginTop:10}}><strong className={String(policyAssessment.data.status)==="blocked"?"neg":String(policyAssessment.data.status)==="warning"?"warn":"pos"}>{String(policyAssessment.data.status).toUpperCase()}</strong><p className="muted">Daily P&amp;L: {String(policyAssessment.data.daily_pnl)} {currency} · Trades today: {String(policyAssessment.data.trades_today)} · Open positions: {String(policyAssessment.data.open_positions)}</p>{Array.isArray(policyAssessment.data.violations)&&policyAssessment.data.violations.map((v,i)=><p key={i} className="muted">{String((v as Record<string,unknown>).severity).toUpperCase()}: {String((v as Record<string,unknown>).message)}</p>)}</div>:policyAssessment.data?<p className="muted">This assessment is stale because calculator inputs or account policy changed. Re-run the account policy check.</p>:null}</>}</div>:null}
 </div>;
}
