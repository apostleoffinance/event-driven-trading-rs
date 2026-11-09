"use client";

import { useMemo, useState } from "react";
import type { JournalAnalytics } from "@/lib/api/journal";

type DayRecord = { pnl: number; trades: number };
const monthNames=["January","February","March","April","May","June","July","August","September","October","November","December"];
const week=["Mon","Tue","Wed","Thu","Fri","Sat","Sun"];
const dateKey=(d:Date)=>d.getUTCFullYear()+"-"+String(d.getUTCMonth()+1).padStart(2,"0")+"-"+String(d.getUTCDate()).padStart(2,"0");
const money=(v:number,c:string)=>v.toLocaleString("en-US",{style:"currency",currency:c,maximumFractionDigits:2});

export function CalendarAnalytics({curve,currency,onSelectDate}:{curve:JournalAnalytics["equity_curve"];currency:string;onSelectDate:(date:string)=>void}){
 const now=new Date();
 const [year,setYear]=useState(now.getUTCFullYear());
 const [month,setMonth]=useState(now.getUTCMonth());
 const daily=useMemo(()=>{
  const map=new Map<string,DayRecord>();
  for(const trade of curve??[]){
   if(!trade.closed_at)continue;
   const key=dateKey(new Date(trade.closed_at));
   const pnl=Number(trade.net_pnl??0);
   if(!Number.isFinite(pnl))continue;
   const existing=map.get(key)??{pnl:0,trades:0};
   existing.pnl+=pnl;existing.trades+=1;map.set(key,existing);
  }
  return map;
 },[curve]);
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
 const changeMonth=(delta:number)=>{const d=new Date(Date.UTC(year,month+delta,1));setYear(d.getUTCFullYear());setMonth(d.getUTCMonth());};
 return <section className="panel calendar-analytics">
  <div className="calendar-analytics-head"><div><span className="metric-label">PERFORMANCE CALENDAR</span><h2>{monthNames[month]} {year}</h2><p className="muted">Daily closed-trade P&amp;L. Click a date to apply it to the Analytics filters.</p></div><div className="calendar-nav"><button type="button" className="btn" onClick={()=>changeMonth(-1)} aria-label="Previous month">‹</button><button type="button" className="btn" onClick={()=>{setYear(now.getUTCFullYear());setMonth(now.getUTCMonth());}}>This month</button><button type="button" className="btn" onClick={()=>changeMonth(1)} aria-label="Next month" disabled={year===now.getUTCFullYear()&&month>=now.getUTCMonth()}>›</button></div></div>
  <div className="grid grid-kpi calendar-month-stats"><div className="panel"><span className="metric-label">Monthly realized P&amp;L</span><strong className={"metric-value "+(total>=0?"pos":"neg")}>{money(total,currency)}</strong></div><div className="panel"><span className="metric-label">Closed trades</span><strong className="metric-value">{tradeCount}</strong></div><div className="panel"><span className="metric-label">Active trading days</span><strong className="metric-value">{monthRecords.length}</strong></div></div>
  <div className="calendar-analytics-grid" role="grid" aria-label={monthNames[month]+" "+year+" daily trading performance"}>{week.map(d=><div className="calendar-weekday" role="columnheader" key={d}>{d}</div>)}{cells.map((key,i)=>{if(!key)return <div className="calendar-day blank" key={"blank-"+i} aria-hidden="true"/>;const d=daily.get(key);const tone=!d?"none":d.pnl>0?"pos":d.pnl<0?"neg":"flat";const alpha=d?Math.max(.12,Math.abs(d.pnl)/maxAbs*.75):0;const style=d?{background:"color-mix(in srgb, "+(d.pnl>=0?"var(--positive)":"var(--negative)")+" "+Math.round(alpha*100)+"%, var(--bg-2))"}:undefined;const isFuture=key>dateKey(now);return <button key={key} type="button" className={"calendar-day "+tone+(isFuture?" future":"")} style={style} disabled={isFuture||!d} onClick={()=>onSelectDate(key)} title={d?key+" · "+money(d.pnl,currency)+" · "+d.trades+" trades":key+" · no closed trades"}><span className="calendar-day-num">{Number(key.slice(-2))}</span>{d?<><strong className={d.pnl>=0?"pos":"neg"}>{money(d.pnl,currency)}</strong><small>{d.trades} trade{d.trades===1?"":"s"}</small></>:<small>—</small>}</button>;})}</div>
  <div className="calendar-legend"><span><i className="pos"/> Profit day</span><span><i className="neg"/> Loss day</span><span>Color intensity reflects daily P&amp;L magnitude</span></div>
  {!curve?.length?<p className="muted">No closed trade history is available for this filter. Add or import closed Journal trades to populate the calendar.</p>:null}
 </section>;
}
