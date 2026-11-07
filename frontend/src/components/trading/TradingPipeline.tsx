import Link from "next/link";
import { healthTone, StatusBadge } from "@/components/data-display/StatusBadge";
import type { PipelineNode } from "@/types/domain";

const statusLabel: Record<PipelineNode["status"], string> = {
  HEALTHY: "Available",
  DEGRADED: "Needs attention",
  DISCONNECTED: "Disconnected",
  ERROR: "Unavailable",
  UNKNOWN: "Status unavailable",
  NOT_CONFIGURED: "Not configured",
};

export function TradingPipeline({ nodes }: { nodes: PipelineNode[] }) {
  if (!nodes.length) {
    return (
      <div className="empty-state">
        <p>No trading activity yet.</p>
        <p className="metric-sub">Your activity will appear here after you connect or create a trading account and start trading.</p>
        <Link href="/accounts" className="btn">Set up a trading account</Link>
      </div>
    );
  }

  return (
    <div className="pipeline" aria-label="Trading activity">
      {nodes.map((node, i) => (
        <div key={node.id} style={{ display: "contents" }}>
          {i > 0 ? <div className="pipeline-arrow" aria-hidden>→</div> : null}
          <Link href={node.href} className="pipeline-node">
            <div className="metric-label">{node.label}</div>
            <div style={{ margin: "6px 0" }}>
              <StatusBadge tone={healthTone(node.status)}>{statusLabel[node.status]}</StatusBadge>
            </div>
            <div className="metric-sub">{node.metric}</div>
            {node.lastActivity ? <div className="metric-sub muted">Last activity {new Date(node.lastActivity).toLocaleString()}</div> : null}
          </Link>
        </div>
      ))}
    </div>
  );
}
