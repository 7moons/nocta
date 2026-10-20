import { ShieldCheck } from "lucide-react";
import type { AccountHealth } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { HealthBar, HealthLegend, OverviewCard } from "./parts";
import type { Overview } from "./overview";

/** Session states of all accounts and how fresh the checks are. */
export function HealthCard({
  data,
  canCheck,
  onOpen,
  onCheckUnchecked,
}: {
  data: Overview;
  canCheck: boolean;
  onOpen: (health: AccountHealth) => void;
  onCheckUnchecked: () => void;
}) {
  const t = useT();
  const total = data.accounts.length;
  return (
    <OverviewCard
      className="overview-health"
      title={t("dashboard.health.title")}
      subtitle={t("dashboard.health.subtitle")}
      action={
        data.neverChecked > 0 && (
          <Button
            variant="outline"
            size="sm"
            disabled={!canCheck}
            onClick={onCheckUnchecked}
          >
            <ShieldCheck />
            {t("dashboard.health.checkUnchecked", {
              count: data.neverChecked,
            })}
          </Button>
        )
      }
    >
      {total === 0 ? (
        <p className="overview-empty">{t("dashboard.health.empty")}</p>
      ) : (
        <>
          <HealthBar counts={data.counts} total={total} large />
          <HealthLegend counts={data.counts} total={total} onOpen={onOpen} />
          <p className="overview-note">
            {t("dashboard.health.freshness", {
              today: data.checkedToday,
              never: data.neverChecked,
            })}
          </p>
        </>
      )}
    </OverviewCard>
  );
}
