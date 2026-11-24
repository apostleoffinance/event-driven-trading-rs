import { Info } from "lucide-react";

type InfoTooltipProps = {
  label: string;
  content: string;
};

export function InfoTooltip({ label, content }: InfoTooltipProps) {
  return (
    <details className="info-tooltip">
      <summary aria-label={`More information about ${label}`} title={`More information about ${label}`}>
        <Info aria-hidden="true" size={14} strokeWidth={1.8} />
      </summary>
      <div className="info-tooltip__panel" role="note">
        <strong>{label}</strong>
        <p>{content}</p>
      </div>
    </details>
  );
}
