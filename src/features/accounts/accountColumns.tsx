import { useEffect, useMemo, useState, type ComponentType } from "react";
import { Ellipsis } from "lucide-react";
import { createColumnHelper } from "@tanstack/react-table";
import type { Account, AccountTask, Folder } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT, type Translate } from "@/i18n";
import { useFormat, type Formatter } from "@/i18n/format";
import { taskActive } from "@/lib/tasks";
import { AccountName } from "./AccountName";
import { HealthBadge } from "./HealthBadge";
import type { QuickActionId } from "./quickActions";

export interface RowAction {
  id: QuickActionId;
  label: string;
  icon: ComponentType;
  disabled?: boolean;
}

const helper = createColumnHelper<Account>();
const PRESENCE = ["offline", "online", "inGame", "inStudio"] as const;

function presenceTitle(account: Account, t: Translate, format: Formatter) {
  const presence = account.presence;
  if (!presence) return undefined;
  const state = PRESENCE[presence.userPresenceType];
  return [
    t(state ? `accounts.presence.${state}` : "accounts.presence.unknown"),
    presence.lastLocation,
    presence.placeId
      ? t("accounts.presence.place", { id: String(presence.placeId) })
      : null,
    presence.lastOnline
      ? t("accounts.presence.lastOnline", {
          date: format.dateTime(presence.lastOnline) ?? "",
        })
      : null,
  ]
    .filter(Boolean)
    .join("\n");
}

function DateCell({
  value,
  now,
  withRelative,
}: {
  value: string | undefined;
  now: number;
  withRelative?: boolean;
}) {
  const t = useT();
  const format = useFormat();
  return (
    <span
      className={`muted tabular ${withRelative ? "date-stack" : ""}`}
      title={format.dateTime(value) ?? t("common.never")}
    >
      {format.date(value)}
      {withRelative && <small>{format.relative(value, now)}</small>}
    </span>
  );
}

/** Columns of the account table. Relative times refresh every minute. */
export function useAccountColumns({
  tasks,
  folders,
  rowActions,
  trash,
  onRowAction,
  onActions,
}: {
  tasks: AccountTask[];
  folders: Folder[];
  rowActions: RowAction[];
  trash: boolean;
  onRowAction: (id: QuickActionId, account: Account) => void;
  onActions: (account: Account) => void;
}) {
  const t = useT();
  const format = useFormat();
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const interval = window.setInterval(() => setNow(Date.now()), 60_000);
    return () => window.clearInterval(interval);
  }, []);
  const accountTasks = useMemo(
    () => new Map(tasks.map((task) => [task.accountId, task])),
    [tasks],
  );
  const folderNames = useMemo(
    () => new Map(folders.map((folder) => [folder.id, folder.name])),
    [folders],
  );

  return useMemo(
    () => [
      helper.display({
        id: "select",
        header: ({ table }) => (
          <input
            type="checkbox"
            aria-label={t("accounts.table.selectAll")}
            checked={
              table.getIsAllRowsSelected() &&
              table.getRowModel().rows.length > 0
            }
            ref={(element) => {
              if (element)
                element.indeterminate = table.getIsSomeRowsSelected();
            }}
            onChange={table.getToggleAllRowsSelectedHandler()}
          />
        ),
        cell: ({ row }) => (
          <input
            type="checkbox"
            aria-label={t("accounts.table.select", {
              name: row.original.username,
            })}
            checked={row.getIsSelected()}
            onChange={row.getToggleSelectedHandler()}
          />
        ),
      }),
      helper.accessor("id", {
        header: t("accounts.table.id"),
        cell: (info) => (
          <span className="muted tabular">{info.getValue()}</span>
        ),
      }),
      helper.accessor("username", {
        header: t("accounts.table.account"),
        cell: (info) => (
          <AccountName
            account={info.row.original}
            title={presenceTitle(info.row.original, t, format)}
          />
        ),
      }),
      helper.accessor("health", {
        header: t("accounts.table.session"),
        cell: (info) => <HealthBadge health={info.getValue()} />,
      }),
      helper.display({
        id: "task",
        header: t("accounts.table.activity"),
        cell: ({ row }) => {
          const task =
            accountTasks.get(row.original.id) ?? row.original.lastTask;
          const tone =
            task?.status === "failed"
              ? "activity-failed"
              : task && taskActive(task)
                ? "activity-active"
                : "";
          return (
            <span
              className={`activity-status ${tone}`}
              title={
                task
                  ? t(
                      task.durationMs === null
                        ? "accounts.table.taskDetail"
                        : "accounts.table.taskDetailTimed",
                      {
                        kind: t(`task.kind.${task.kind}`),
                        detail: task.detail,
                        ms: task.durationMs ?? 0,
                      },
                    )
                  : undefined
              }
            >
              <span className="activity-dot" />
              {task ? t(`task.status.${task.status}`) : t("task.idle")}
            </span>
          );
        },
      }),
      helper.accessor("folderId", {
        header: t("accounts.table.folder"),
        cell: (info) => (
          <span className="muted">
            {folderNames.get(info.getValue() ?? -1) ??
              t("accounts.folders.general")}
          </span>
        ),
      }),
      helper.accessor((account) => account.lastStartedAt ?? undefined, {
        id: "lastStartedAt",
        header: t("accounts.table.lastStarted"),
        sortUndefined: "last",
        cell: (info) => (
          <DateCell value={info.getValue()} now={now} withRelative />
        ),
      }),
      helper.accessor(
        (account) =>
          (trash ? account.deletedAt : account.checkedAt) ?? undefined,
        {
          id: trash ? "deletedAt" : "checkedAt",
          sortUndefined: "last",
          header: t(
            trash ? "accounts.table.deleted" : "accounts.table.lastChecked",
          ),
          cell: (info) => <DateCell value={info.getValue()} now={now} />,
        },
      ),
      helper.display({
        id: "actions",
        header: "",
        cell: ({ row }) => (
          <div
            className="row-actions"
            onDoubleClick={(event) => event.stopPropagation()}
          >
            {rowActions.map((action) => (
              <Button
                key={action.id}
                variant="ghost"
                size="icon"
                className={`row-action row-action-${action.id}`}
                aria-label={`${action.label}: ${row.original.username}`}
                title={action.label}
                disabled={action.disabled}
                onClick={() => onRowAction(action.id, row.original)}
              >
                <action.icon />
              </Button>
            ))}
            <Button
              variant="ghost"
              size="icon"
              aria-label={t("accounts.table.actionsFor", {
                name: row.original.username,
              })}
              title={t("accounts.table.moreActions")}
              onClick={() => onActions(row.original)}
            >
              <Ellipsis />
            </Button>
          </div>
        ),
      }),
    ],
    [
      t,
      format,
      folderNames,
      onActions,
      rowActions,
      onRowAction,
      trash,
      accountTasks,
      now,
    ],
  );
}
