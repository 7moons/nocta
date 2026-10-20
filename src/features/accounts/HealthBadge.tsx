import { Circle, ShieldCheck, ShieldX } from "lucide-react";
import type { AccountHealth } from "@/generated/contracts";
import { useT } from "@/i18n";

/** The result of the last session check, with an icon. */
export function HealthBadge({ health }: { health: AccountHealth }) {
  const t = useT();
  const Icon =
    health === "healthy"
      ? ShieldCheck
      : health === "invalid_session"
        ? ShieldX
        : Circle;
  return (
    <span className={`health health-${health}`}>
      <Icon />
      {t(`health.${health}`)}
    </span>
  );
}
