import type { HTMLAttributes, ReactNode } from "react";

type GradientAccountCardProps = HTMLAttributes<HTMLElement> & {
  eyebrow?: string;
  title: string;
  children: ReactNode;
};

export function GradientAccountCard({
  eyebrow,
  title,
  children,
  className,
  ...props
}: GradientAccountCardProps) {
  const classes = ["gradient-account-card", className].filter(Boolean).join(" ");

  return (
    <section className={classes} aria-label={title} {...props}>
      {eyebrow ? <p className="gradient-account-card__eyebrow">{eyebrow}</p> : null}
      <h2 className="gradient-account-card__title">{title}</h2>
      <div className="gradient-account-card__content">{children}</div>
    </section>
  );
}
