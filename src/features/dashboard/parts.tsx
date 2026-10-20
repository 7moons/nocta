import type { ReactNode } from "react";
import type { AccountHealth } from "@/generated/contracts";
import { useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import { HEALTH_ORDER, type HealthCounts } from "./overview";

/** A titled panel of the overview with an optional action in its header. */
export function OverviewCard({
  title,
  subtitle,
  action,
  className = "",
  children,
}: {
  title: string;
  subtitle?: string;
  action?: ReactNode;
  className?: string;
  children: ReactNode;
}) {
  return (
    <section className={`overview-card ${className}`} aria-label={title}>
      <header>
        <div>
          <h3>{title}</h3>
          {subtitle && <p>{subtitle}</p>}
        </div>
        {action}
      </header>
      {children}
    </section>
  );
}

/** Accounts split by session state, as one bar. */
export function HealthBar({
  counts,
  total,
  large = false,
}: {
  counts: HealthCounts;
  total: number;
  large?: boolean;
}) {
  const t = useT();
  return (
    <div
      className={`health-bar ${large ? "large" : ""}`}
      role="img"
      aria-label={HEALTH_ORDER.map(
        (health) => `${t(`health.${health}`)}: ${counts[health]}`,
      ).join(", ")}
    >
      {total > 0 &&
        HEALTH_ORDER.map(
          (health) =>
            counts[health] > 0 && (
              <span
                key={health}
                className={`health-bar-${health}`}
                style={{ flexGrow: counts[health] }}
              />
            ),
        )}
    </div>
  );
}

/** Legend for a health bar; each state can open the accounts filtered to it. */
export function HealthLegend({
  counts,
  total,
  onOpen,
}: {
  counts: HealthCounts;
  total: number;
  onOpen: (health: AccountHealth) => void;
}) {
  const t = useT();
  const format = useFormat();
  return (
    <ul className="health-legend">
      {HEALTH_ORDER.map((health) => (
        <li key={health}>
          <button
            onClick={() => onOpen(health)}
            disabled={counts[health] === 0}
          >
            <span className={`health-dot health-bar-${health}`} />
            <span>{t(`health.${health}`)}</span>
            <strong>{format.number(counts[health])}</strong>
            <small>
              {total > 0
                ? `${Math.round((counts[health] / total) * 100)}%`
                : "—"}
            </small>
          </button>
        </li>
      ))}
    </ul>
  );
}
