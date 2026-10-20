import { LoaderCircle } from "lucide-react";
import type { AppSnapshot } from "@/generated/contracts";
import { useT } from "@/i18n";
import { desktop } from "@/lib/ipc";
import { taskActive } from "@/lib/tasks";

export function StatusBar({
  snapshot,
  pending,
  onChangelog,
}: {
  snapshot: AppSnapshot;
  pending: boolean;
  onChangelog: () => void;
}) {
  const t = useT();
  const accounts = snapshot.accounts.filter(
    (account) => !account.deletedAt,
  ).length;
  return (
    <footer className="statusbar">
      <span>
        <span className={`status-dot ${desktop ? "ready" : ""}`} />
        {t(desktop ? "app.status.local" : "app.status.preview")}
      </span>
      <span>
        {pending ? (
          <>
            <LoaderCircle className="spin" size={11} />
            {t("common.saving")}
          </>
        ) : (
          t("app.status.ready")
        )}
      </span>
      <span className="statusbar-end">
        {t("accounts.count", { count: accounts })}
        <span>•</span>
        {t("app.status.activeTasks", {
          count: snapshot.tasks.filter(taskActive).length,
        })}
        <span>•</span>
        <button
          type="button"
          className="statusbar-version"
          title={t("changelog.open")}
          onClick={onChangelog}
        >
          v{snapshot.runtime.version}
        </button>
      </span>
    </footer>
  );
}
