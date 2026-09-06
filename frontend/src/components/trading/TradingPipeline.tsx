import Link from "next/link";
import { healthTone, StatusBadge } from "@/components/data-display/StatusBadge";
import type { PipelineNode } from "@/types/domain";

export function TradingPipeline({ nodes }: { nodes: PipelineNode[] }) {
  return (
    <div className="pipeline" aria-label="Trading pipeline">
      {nodes.map((node, i) => (
        <div key={node.id} style={{ display: "contents" }}>
          {i > 0 ? <div className="pipeline-arrow" aria-hidden>→</div> : null}
          <Link href={node.href} className="pipeline-node">
            <div className="metric-label">{node.label}</div>
            <div style={{ margin: "6px 0" }}>
              <StatusBadge tone={healthTone(node.status)}>
                {node.status}
              </StatusBadge>
            </div>
            <div className="metric-sub">{node.metric}</div>
            <div className="metric-sub muted">{node.lastActivity}</div>
          </Link>
        </div>
      ))}
    </div>
  );
}
