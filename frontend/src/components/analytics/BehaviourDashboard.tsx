"use client";

import type { CSSProperties } from "react";
import type { JournalIntelligence } from "@/lib/api/journal";
import styles from "./BehaviourDashboard.module.css";

type Props = { data: JournalIntelligence };

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

export function BehaviourDashboard({ data }: Props) {
  const outcomes = data.process_outcomes;
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
    <p className={styles.notice}>{data.notice}</p>
  </section>;
}
