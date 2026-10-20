import type { AccountTask } from "@/generated/contracts";
import { useT } from "@/i18n";
import { taskActive } from "@/lib/tasks";

const BULK_KINDS = ["launch_game", "open_browser", "refresh_cookie"];
const SHOWN = 3;

// A manual browser waiting for the user is not bulk work in progress.
export const isBulkWork = (task: AccountTask) =>
  BULK_KINDS.includes(task.kind) &&
  taskActive(task) &&
  !(task.kind === "open_browser" && task.status === "waiting_user");

/** What a bulk launch or browser run is doing now and what comes next. */
export function BulkProgress({
  tasks,
  accountNames,
}: {
  tasks: AccountTask[];
  accountNames: Map<number, string>;
}) {
  const t = useT();
  const work = tasks.filter(isBulkWork);
  if (work.length === 0) return null;
  const current = work.filter((task) => task.status !== "queued");
  const queued = work.filter((task) => task.status === "queued");
  const next = queued.reduce<AccountTask | undefined>(
    (first, task) => (!first || task.id < first.id ? task : first),
    undefined,
  );
  const describe = (task: AccountTask) =>
    t("accounts.bulk.task", {
      account:
        accountNames.get(task.accountId) ??
        t("accounts.bulk.unknownAccount", { id: String(task.accountId) }),
      kind: t(`task.kind.${task.kind}`),
    });
  const shown = current.slice(0, SHOWN).map(describe).join(", ");
  return (
    <div
      className="bulk-progress"
      role="status"
      aria-label={t("accounts.bulk.label")}
    >
      <span className="badge">
        {t("accounts.bulk.counts", {
          active: current.length,
          queued: queued.length,
        })}
      </span>
      <span title={current.map(describe).join(", ")}>
        {t("accounts.bulk.current", {
          tasks: shown || t("accounts.bulk.preparing"),
        })}
        {current.length > SHOWN && ` +${current.length - SHOWN}`}
      </span>
      <span title={next ? describe(next) : undefined}>
        {t("accounts.bulk.next", {
          task: next ? describe(next) : t("accounts.bulk.none"),
        })}
      </span>
    </div>
  );
}
