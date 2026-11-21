import type { HTMLAttributes } from "react";

type SurfaceCardProps = HTMLAttributes<HTMLDivElement> & {
  raised?: boolean;
};

export function SurfaceCard({
  raised = false,
  className,
  children,
  ...props
}: SurfaceCardProps) {
  const classes = ["surface-card", raised ? "surface-card--raised" : "", className]
    .filter(Boolean)
    .join(" ");

  return (
    <div className={classes} {...props}>
      {children}
    </div>
  );
}
