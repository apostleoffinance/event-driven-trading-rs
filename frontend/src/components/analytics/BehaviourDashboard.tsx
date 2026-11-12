"use client";

import type { CSSProperties } from "react";
import type { JournalAnalytics, JournalIntelligence } from "@/lib/api/journal";
import styles from "./BehaviourDashboard.module.css";

type BehaviourTrade = NonNullable<JournalAnalytics["trade_metrics"]>[number];
type Props = { data: JournalIntelligence; trades: BehaviourTrade[]; onSelectGroup: (dimension: "Session" | "Setup" | "Weekday (UTC)" | "Month (UTC)", key: string) => void };

function formatPercent(value: number | null) {
  return value == null || !Number.isFinite(value) ? "—" : `${value.toFixed(1)}%`;
}

function MetricCard({ label, value, detail, progress, tone = "navy" }: {
  label: string; value: string; detail: string; progress?: number | null; tone?: "navy" | "orange" | "green";
}) {
  const width = progress == null || !Number.isFinite(progress) ? 0 : Math.max(0, Math.min(100, progress));
  return <article className={styles.metricCard}>
    <span className={styles.metricLabel}>{label}</span>
    <strong className={styles.metricValue}>{value}</strong>
    <span className={styles.metricDetail}>{detail}</span>
    {progress != null ? <div className={styles.progressTrack} role="progressbar" aria-label={label} aria-valuemin={0} aria-valuemax={100} aria-valuenow={width}>
      <span className={`${styles.progressFill} ${styles[tone]}`} style={{ width: `${width}%` }} />
    </div> : null}
  </article>;
}

function OutcomeRow({ label, wins, losses, good }: {
  label: string; wins: number; losses: number; good: boolean;
}) {
  const rowTotal = wins + losses;
  const winWidth = rowTotal ? wins / rowTotal * 100 : 0;
  const lossWidth = rowTotal ? losses / rowTotal * 100 : 0;
  return <div className={styles.outcomeRow}>
    <div className={styles.outcomeHeading}>
      <span className={styles.outcomeName}><i className={good ? styles.goodDot : styles.poorDot} />{label}</span>
      <span className={styles.outcomeTotal}>{rowTotal} {rowTotal === 1 ? "trade" : "trades"}</span>
    </div>
    {rowTotal > 0 ? <div className={styles.outcomeTrack} aria-label={`${label}: ${wins} wins and ${losses} losses`}>
      {wins > 0 ? <span className={styles.winSegment} style={{ width: `${winWidth}%` } as CSSProperties}>{wins} {wins === 1 ? "win" : "wins"}</span> : null}
      {losses > 0 ? <span className={styles.lossSegment} style={{ width: `${lossWidth}%` } as CSSProperties}>{losses} {losses === 1 ? "loss" : "losses"}</span> : null}
    </div> : <div className={styles.emptyTrack}>No trades recorded in this category</div>}
  </div>;
}

export function BehaviourDashboard({ data, trades, onSelectGroup }: Props) {
  const outcomes = data.process_outcomes;
  const monthlyTrend = new Map<string,{trades:number;rules:number;rulesKnown:number;valid:number;validKnown:number;emotional:number;discipline:number;disciplineKnown:number}>();
  for (const trade of trades) {
    const month=trade.closed_at.slice(0,7);
    const row=monthlyTrend.get(month)??{trades:0,rules:0,rulesKnown:0,valid:0,validKnown:0,emotional:0,discipline:0,disciplineKnown:0};
    row.trades+=1;
    if(trade.rules_followed!=null){row.rulesKnown+=1;if(trade.rules_followed)row.rules+=1;}
    if(trade.setup_valid!=null){row.validKnown+=1;if(trade.setup_valid)row.valid+=1;}
    if(trade.emotional_trade===true)row.emotional+=1;
    if(trade.discipline_score!=null){row.discipline+=trade.discipline_score;row.disciplineKnown+=1;}
    monthlyTrend.set(month,row);
  }
  const segmentMap = new Map<string,{dimension:string;label:string;trades:number;netPnl:number;realizedR:number;rCount:number;rules:number;rulesKnown:number;valid:number;validKnown:number;emotional:number;discipline:number;disciplineKnown:number}>();
  for (const trade of trades) {
    const closed = new Date(trade.closed_at);
    const weekday = Number.isNaN(closed.getTime()) ? "Unknown" : ["Sunday","Monday","Tuesday","Wednesday","Thursday","Friday","Saturday"][closed.getUTCDay()];
    const dimensions: Array<[string,string]> = [["Session",trade.session??"Unspecified"],["Setup",trade.setup??"Unassigned"],["Weekday (UTC)",weekday]];
    for (const [dimension,label] of dimensions) {
      const key=dimension+"::"+label;
      const row=segmentMap.get(key)??{dimension,label,trades:0,netPnl:0,realizedR:0,rCount:0,rules:0,rulesKnown:0,valid:0,validKnown:0,emotional:0,discipline:0,disciplineKnown:0};
      row.trades+=1;
      const pnl=Number(trade.net_pnl);if(Number.isFinite(pnl))row.netPnl+=pnl;
      if(trade.realized_r!=null&&Number.isFinite(Number(trade.realized_r))){row.realizedR+=Number(trade.realized_r);row.rCount+=1;}
      if(trade.rules_followed!=null){row.rulesKnown+=1;if(trade.rules_followed)row.rules+=1;}
      if(trade.setup_valid!=null){row.validKnown+=1;if(trade.setup_valid)row.valid+=1;}
      if(trade.emotional_trade===true)row.emotional+=1;
      if(trade.discipline_score!=null){row.discipline+=trade.discipline_score;row.disciplineKnown+=1;}
      segmentMap.set(key,row);
    }
  }
  const segmentRows=[...segmentMap.values()].sort((a,b)=>a.dimension.localeCompare(b.dimension)||a.label.localeCompare(b.label));
  const total = outcomes.good_process_win + outcomes.good_process_loss + outcomes.poor_process_win + outcomes.poor_process_loss;
  const confidence = data.win_rate_confidence_95;
  const sampleLabel = data.sample_status.replaceAll("_", " ");
  const discipline = data.average_discipline_score;
  const emotional = data.emotional_trade_count;
  return <section className={styles.dashboard} aria-labelledby="behaviour-dashboard-title">
    <div className={styles.header}>
      <div>
        <span className={styles.eyebrow}>PROCESS &amp; STATISTICAL INTELLIGENCE</span>
        <h2 id="behaviour-dashboard-title" className={styles.title}>Your process, at a glance</h2>
        <p className={styles.subtitle}>Measure the quality of your decisions separately from the outcome of each trade.</p>
      </div>
      <div className={styles.samplePill}><span className={styles.sampleDot} />{sampleLabel} <b>·</b> n={data.trade_count}</div>
    </div>

    <div className={styles.metricGrid}>
      <MetricCard label="Rule adherence" value={formatPercent(data.rule_adherence_percent)} detail={`${data.rule_adherence_known_count} trades with a recorded rule decision`} progress={data.rule_adherence_percent} tone="green" />
      <MetricCard label="Valid setup rate" value={formatPercent(data.valid_setup_percent)} detail={`${data.valid_setup_known_count} trades with setup validity recorded`} progress={data.valid_setup_percent} tone="navy" />
      <MetricCard label="Average discipline" value={discipline == null ? "—" : `${discipline.toFixed(1)}/100`} detail={discipline == null ? "No discipline scores recorded" : "Average recorded discipline score"} progress={discipline} tone="orange" />
      <MetricCard label="Emotional trades" value={String(emotional)} detail={`${data.mistake_count} recorded ${data.mistake_count === 1 ? "mistake" : "mistakes"} in this sample`} />
    </div>

    <div className={styles.outcomePanel}>
      <div className={styles.outcomeHeader}>
        <div>
          <h3 className={styles.sectionTitle}>Process vs. outcome</h3>
          <p className={styles.sectionDescription}>See whether trades followed your rules, regardless of whether they won or lost.</p>
        </div>
        <span className={styles.totalBadge}>{total} classified trades</span>
      </div>
      <div className={styles.legend}>
        <span><i className={styles.legendWin} />Win</span>
        <span><i className={styles.legendLoss} />Loss</span>
      </div>
      <div className={styles.rows}>
        <OutcomeRow label="Good process" wins={outcomes.good_process_win} losses={outcomes.good_process_loss} good />
        <OutcomeRow label="Poor process" wins={outcomes.poor_process_win} losses={outcomes.poor_process_loss} good={false} />
      </div>
      <p className={styles.outcomeFootnote}>A good-process loss can still be a well-executed trade. A winning trade does not automatically mean the process was good.</p>
    </div>

    <div className={styles.confidencePanel}>
      <div>
        <span className={styles.confidenceLabel}>WIN-RATE ESTIMATE · 95% WILSON INTERVAL</span>
        <strong className={styles.confidenceValue}>{data.win_rate_percent == null ? "—" : `${Number(data.win_rate_percent).toFixed(1)}%`}</strong>
        <span className={styles.confidenceDetail}>{confidence ? `${confidence.lower_percent.toFixed(1)}% – ${confidence.upper_percent.toFixed(1)}%` : "Confidence interval unavailable"}</span>
      </div>
      <p>Statistical context, not a prediction. The interval communicates uncertainty in the observed win rate; it is not a probability that the true rate lies inside these bounds.</p>
    </div>
    {monthlyTrend.size ? <div className={styles.outcomePanel}>
      <h3 className={styles.sectionTitle}>Behaviour trends by month</h3>
      <p className={styles.sectionDescription}>Monthly aggregates from recorded rule, setup, emotion and discipline fields. Unrecorded values are excluded from each denominator.</p>
      <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Month</th><th>Trades</th><th>Rule adherence</th><th>Valid setups</th><th>Emotional</th><th>Avg discipline</th></tr></thead><tbody>{[...monthlyTrend.entries()].sort(([a],[b])=>a.localeCompare(b)).map(([month,row])=><tr key={month}><td><button type="button" className="btn" onClick={()=>onSelectGroup("Month (UTC)",month)}>{month}</button></td><td>{row.trades}</td><td>{row.rulesKnown?(row.rules/row.rulesKnown*100).toFixed(1)+"%": "—"} ({row.rulesKnown})</td><td>{row.validKnown?(row.valid/row.validKnown*100).toFixed(1)+"%":"—"} ({row.validKnown})</td><td>{row.emotional}</td><td>{row.disciplineKnown?(row.discipline/row.disciplineKnown).toFixed(1)+"/100":"—"}</td></tr>)}</tbody></table></div>
    </div> : null}
    {segmentRows.length ? <div className={styles.outcomePanel}>
      <h3 className={styles.sectionTitle}>Behaviour by session, setup and weekday</h3>
      <p className={styles.sectionDescription}>Compare process quality with realized outcomes across the same trade population. Percentages use only trades with recorded values; these are descriptive associations, not causal conclusions.</p>
      <div className="data-table-wrap"><table className="data-table"><thead><tr><th>Dimension</th><th>Group</th><th>Trades</th><th>Net P&amp;L</th><th>Avg R</th><th>Rule adherence</th><th>Valid setups</th><th>Emotional</th><th>Avg discipline</th></tr></thead><tbody>{segmentRows.map(row=><tr key={row.dimension+"::"+row.label}><td>{row.dimension}</td><td><button type="button" className="btn" onClick={()=>onSelectGroup(row.dimension as "Session" | "Setup" | "Weekday (UTC)",row.label)}>{row.label}</button></td><td>{row.trades}</td><td className={row.netPnl>=0?"num pos":"num neg"}>{row.netPnl.toFixed(2)}</td><td>{row.rCount?(row.realizedR/row.rCount).toFixed(2)+"R":"—"}</td><td>{row.rulesKnown?(row.rules/row.rulesKnown*100).toFixed(1)+"% ("+row.rulesKnown+")":"—"}</td><td>{row.validKnown?(row.valid/row.validKnown*100).toFixed(1)+"% ("+row.validKnown+")":"—"}</td><td>{row.emotional}</td><td>{row.disciplineKnown?(row.discipline/row.disciplineKnown).toFixed(1)+"/100":"—"}</td></tr>)}</tbody></table></div>
    </div> : null}
    <div className={styles.confidencePanel}>
      <div>
        <span className={styles.confidenceLabel}>RECORDED PSYCHOLOGY</span>
        <strong className={styles.confidenceValue}>{data.psychology_record_count ?? 0}/{data.trade_count}</strong>
        <span className={styles.confidenceDetail}>Trades with psychology records</span>
      </div>
      <div>
        <span className={styles.confidenceDetail}>Confidence {data.average_confidence == null ? "—" : data.average_confidence.toFixed(1)+"/10"} · Fear {data.average_fear == null ? "—" : data.average_fear.toFixed(1)+"/10"} · FOMO {data.average_fomo == null ? "—" : data.average_fomo.toFixed(1)+"/10"}</span>
        <span className={styles.confidenceDetail}>Frustration {data.average_frustration == null ? "—" : data.average_frustration.toFixed(1)+"/10"} · Revenge {data.average_revenge_intensity == null ? "—" : data.average_revenge_intensity.toFixed(1)+"/10"} · Boredom {data.average_boredom == null ? "—" : data.average_boredom.toFixed(1)+"/10"}</span>
      </div>
    </div>
    {data.insights?.length ? <div className={styles.outcomePanel}>
      <h3 className={styles.sectionTitle}>Diagnostic insights</h3>
      <p className={styles.sectionDescription}>Rule-based observations from recorded Journal facts. These are descriptive prompts, not causal conclusions.</p>
      <ul>{data.insights.map((insight,index)=><li key={insight.type+"-"+index}><strong>{insight.title}</strong> — {insight.message}</li>)}</ul>
    </div> : <div className={styles.outcomePanel}><h3 className={styles.sectionTitle}>No diagnostic alerts</h3><p className={styles.sectionDescription}>There are no current rule-based observations for this sample, or there is not enough recorded data to evaluate them.</p></div>}
    <p className={styles.notice}>{data.notice}</p>
  </section>;
}
