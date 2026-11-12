"use client";

import { useMemo, useState } from "react";
import type { JournalAnalytics } from "@/lib/api/journal";

type CalendarTrade = NonNullable<JournalAnalytics["trade_metrics"]>[number];
type DayRecord = { pnl: number; trades: number; wins: number; losses: number; totalR:number; rCount:number; bestTrade:number; worstTrade:number };
const monthNames=["January","February","March","April","May","June","July","August","September","October","November","December"];
const week=["Mon","Tue","Wed","Thu","Fri","Sat","Sun"];
const dateKey=(d:Date)=>d.getUTCFullYear()+"-"+String(d.getUTCMonth()+1).padStart(2,"0")+"-"+String(d.getUTCDate()).padStart(2,"0");
const money=(v:number,c:string)=>v.toLocaleString("en-US",{style:"currency",currency:c,maximumFractionDigits:2});

export function CalendarAnalytics({curve,trades,currency,onSelectDate}:{curve:JournalAnalytics["equity_curve"];trades:CalendarTrade[];currency:string;onSelectDate:(date:string)=>void}){
 const now=new Date();
 const [year,setYear]=useState(now.getUTCFullYear());
 const [month,setMonth]=useState(now.getUTCMonth());
 const daily=useMemo(()=>{
  const map=new Map<string,DayRecord>();
  const source = trades.length ? trades : (curve??[]).map(t=>({closed_at:t.closed_at,net_pnl:t.net_pnl,realized_r:null}));
  for(const trade of source){
   if(!trade.closed_at)continue;
   const key=dateKey(new Date(trade.closed_at));
   const pnl=Number(trade.net_pnl??0);
   if(!Number.isFinite(pnl))continue;
   const existing=map.get(key)??{pnl:0,trades:0,wins:0,losses:0,totalR:0,rCount:0,bestTrade:pnl,worstTrade:pnl};
   existing.pnl+=pnl;existing.trades+=1;existing.bestTrade=Math.max(existing.bestTrade,pnl);existing.worstTrade=Math.min(existing.worstTrade,pnl);
   if(pnl>0)existing.wins+=1;else if(pnl<0)existing.losses+=1;
   const r=trade.realized_r==null?null:Number(trade.realized_r);if(r!=null&&Number.isFinite(r)){existing.totalR+=r;existing.rCount+=1;}
   map.set(key,existing);
  }
  return map;
 },[curve,trades]);
 const first=new Date(Date.UTC(year,month,1));
 const offset=(first.getUTCDay()+6)%7;
 const daysInMonth=new Date(Date.UTC(year,month+1,0)).getUTCDate();
 const cells=Array.from({length:Math.ceil((offset+daysInMonth)/7)*7},(_,i)=>{
  const day=i-offset+1;
  return day>0&&day<=daysInMonth?dateKey(new Date(Date.UTC(year,month,day))):null;
 });
 const monthRecords=[...daily.entries()].filter(([key])=>key.startsWith(year+"-"+String(month+1).padStart(2,"0")));
 const total=monthRecords.reduce((sum,[,d])=>sum+d.pnl,0);
 const tradeCount=monthRecords.reduce((sum,[,d])=>sum+d.trades,0);
 const maxAbs=Math.max(1,...monthRecords.map(([,d])=>Math.abs(d.pnl)));
 const weekly=useMemo(()=>{
  const groups=new Map<string,{pnl:number;trades:number;wins:number;losses:number}>();
  for(const [key,day] of monthRecords){
   const date=new Date(key+"T00:00:00Z");
   const monday=new Date(date);monday.setUTCDate(date.getUTCDate()-((date.getUTCDay()+6)%7));
   const weekKey=dateKey(monday);
   const current=groups.get(weekKey)??{pnl:0,trades:0,wins:0,losses:0};
   current.pnl+=day.pnl;current.trades+=day.trades;current.wins+=day.wins;current.losses+=day.losses;groups.set(weekKey,current);
  }
  return [...groups.entries()].sort(([a],[b])=>a.localeCompare(b));
 },[monthRecords]);
 const bestDay=monthRecords.length?monthRecords.reduce((best,item)=>item[1].pnl>best[1].pnl?item:best):null;
 const monthly=useMemo(()=>{
  const groups=new Map<string,{pnl:number;trades:number;wins:number;totalR:number;rCount:number}>();
  for(const trade of trades){const key=trade.closed_at.slice(0,7);const pnl=Number(trade.net_pnl);const row=groups.get(key)??{pnl:0,trades:0,wins:0,totalR:0,rCount:0};row.pnl+=pnl;row.trades+=1;if(pnl>0)row.wins+=1;if(trade.realized_r!=null&&Number.isFinite(Number(trade.realized_r))){row.totalR+=Number(trade.realized_r);row.rCount+=1;}groups.set(key,row);}
  return [...groups.entries()].sort(([a],[b])=>a.localeCompare(b));
 },[trades]);
 const worstDay=monthRecords.length?monthRecords.reduce((worst,item)=>item[1].pnl<worst[1].pnl?item:worst):null;
 const changeMonth=(delta:number)=>{const d=new Date(Date.UTC(year,month+delta,1));setYear(d.getUTCFullYear());setMonth(d.getUTCMonth());};
 return <section className="panel calendar-analytics">
  <div className="calendar-analytics-head"><div><span className="metric-label">PERFORMANCE CALENDAR</span><h2>{monthNames[month]} {year}</h2><p className="muted">Daily closed-trade P&amp;L. Click a date to apply it to the Analytics filters.</p></div><div className="calendar-nav"><button type="button" className="btn" onClick={()=>changeMonth(-1)} aria-label="Previous month">‹</button><button type="button" className="btn" onClick={()=>{setYear(now.getUTCFullYear());setMonth(now.getUTCMonth());}}>This month</button><button type="button" className="btn" onClick={()=>changeMonth(1)} aria-label="Next month" disabled={year===now.getUTCFullYear()&&month>=now.getUTCMonth()}>›</button></div></div>
  <div className="grid grid-kpi calendar-month-stats"><div className="panel"><span className="metric-label">Monthly realized P&amp;L</span><strong className={"metric-value "+(total>=0?"pos":"neg")}>{money(total,currency)}</strong></div><div className="panel"><span className="metric-label">Closed trades</span><strong className="metric-value">{tradeCount}</strong></div><div className="panel"><span className="metric-label">Active trading days</span><strong className="metric-value">{monthRecords.length}</strong></div><div className="panel"><span className="metric-label">Best day</span><strong className="metric-value pos">{bestDay?money(bestDay[1].pnl,currency):"—"}</strong><small>{bestDay?.[0]??"No data"}</small></div><div className="panel"><span className="metric-label">Worst day</span><strong className="metric-value neg">{worstDay?money(worstDay[1].pnl,currency):"—"}</strong><small>{worstDay?.[0]??"No data"}</small></div></div>
  <div className="calendar-analytics-grid" role="grid" aria-label={monthNames[month]+" "+year+" daily trading performance"}>{week.map(d=><div className="calendar-weekday" role="columnheader" key={d}>{d}</div>)}{cells.map((key,i)=>{if(!key)return <div className="calendar-day blank" key={"blank-"+i} aria-hidden="true"/>;const d=daily.get(key);const tone=!d?"none":d.pnl>0?"pos":d.pnl<0?"neg":"flat";const alpha=d?Math.max(.12,Math.abs(d.pnl)/maxAbs*.75):0;const style=d?{background:"color-mix(in srgb, "+(d.pnl>=0?"var(--positive)":"var(--negative)")+" "+Math.round(alpha*100)+"%, var(--bg-2))"}:undefined;const isFuture=key>dateKey(now);return <button key={key} type="button" className={"calendar-day "+tone+(isFuture?" future":"")} style={style} disabled={isFuture||!d} onClick={()=>onSelectDate(key)} title={d?key+" · "+money(d.pnl,currency)+" · "+d.trades+" trades":key+" · no closed trades"}><span className="calendar-day-num">{Number(key.slice(-2))}</span>{d?<><strong className={d.pnl>=0?"pos":"neg"}>{money(d.pnl,currency)}</strong><small>{d.trades} trade{d.trades===1?"":"s"} · {d.trades?(d.wins/d.trades*100).toFixed(0):0}% wins · Avg R {d.rCount?(d.totalR/d.rCount).toFixed(2):"—"}</small></>:<small>—</small>}</button>;})}</div>
  <section style={{marginTop:20}}><h3>Weekly performance</h3><p className="muted">Weeks begin Monday (UTC). Win rate is based on closed trades in the selected calendar month.</p><div className="data-table-wrap"><table className="data-table"><thead><tr><th>Week of</th><th>Trades</th><th>Wins</th><th>Losses</th><th>Win rate</th><th>Net P&amp;L</th></tr></thead><tbody>{weekly.map(([key,w])=><tr key={key}><td>{key}</td><td>{w.trades}</td><td>{w.wins}</td><td>{w.losses}</td><td>{w.trades?((w.wins/w.trades)*100).toFixed(1)+"%":"—"}</td><td className={w.pnl>=0?"num pos":"num neg"}>{money(w.pnl,currency)}</td></tr>)}</tbody></table>{weekly.length===0?<p className="muted">No weekly activity for this month.</p>:null}</div></section>
  <section style={{marginTop:20}}><h3>Monthly comparison</h3><p className="muted">Only months present in the currently selected Analytics date range are included.</p><div className="data-table-wrap"><table className="data-table"><thead><tr><th>Month</th><th>Trades</th><th>Win rate</th><th>Average R</th><th>Net P&amp;L</th></tr></thead><tbody>{monthly.map(([key,m])=><tr key={key}><td>{key}</td><td>{m.trades}</td><td>{(m.wins/m.trades*100).toFixed(1)}%</td><td>{m.rCount?(m.totalR/m.rCount).toFixed(2)+"R":"—"}</td><td className={m.pnl>=0?"num pos":"num neg"}>{money(m.pnl,currency)}</td></tr>)}</tbody></table>{monthly.length===0?<p className="muted">No monthly records available.</p>:null}</div></section>
  <div className="calendar-legend"><span><i className="pos"/> Profit day</span><span><i className="neg"/> Loss day</span><span>Color intensity reflects daily P&amp;L magnitude</span></div>
  {!curve?.length?<p className="muted">No closed trade history is available for this filter. Add or import closed Journal trades to populate the calendar.</p>:null}
 </section>;
}
