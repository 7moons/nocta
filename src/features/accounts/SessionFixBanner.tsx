import { useMemo, useState } from "react";
import { Wrench, X } from "lucide-react";
import type { Account } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { sessionStale } from "./accountDetails";

/**
 * Points out sessions that need attention and fixes them in one click: expired sessions get a
 * fresh cookie, old ones are checked again.
 */
export function SessionFixBanner({
  accounts,
  disabled,
  onFix,
}: {
  accounts: Account[];
  disabled: boolean;
  onFix: (expired: number[], stale: number[]) => void;
}) {
  const t = useT();
  const [dismissed, setDismissed] = useState(false);
  const { expired, stale } = useMemo(() => {
    const active = accounts.filter((account) => !account.deletedAt);
    return {
      expired: active
        .filter(
          (account) =>
            account.health === "invalid_session" && account.hasPassword,
        )
        .map((account) => account.id),
      stale: active
        .filter((account) => sessionStale(account))
        .map((account) => account.id),
    };
  }, [accounts]);
  if (dismissed || (expired.length === 0 && stale.length === 0)) return null;
  return (
    <div className="session-fix" role="status">
      <Wrench size={15} />
      <span>
        {[
          expired.length > 0 &&
            t("accounts.fix.expired", { count: expired.length }),
          stale.length > 0 && t("accounts.fix.stale", { count: stale.length }),
        ]
          .filter(Boolean)
          .join(" · ")}
      </span>
      <Button
        size="sm"
        disabled={disabled}
        title={t("accounts.fix.detail")}
        onClick={() => {
          onFix(expired, stale);
          setDismissed(true);
        }}
      >
        {t("accounts.fix.button")}
      </Button>
      <Button
        variant="ghost"
        size="icon"
        aria-label={t("accounts.fix.dismiss")}
        onClick={() => setDismissed(true)}
      >
        <X />
      </Button>
    </div>
  );
}
