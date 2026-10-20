import { useMemo, useState } from "react";
import {
  Plus,
  Radio,
  RefreshCw,
  Search,
  ShieldCheck,
  Square,
  Trash2,
  Users,
  X,
} from "lucide-react";
import type {
  Account,
  AccountHealth,
  AppConfig,
  AppSnapshot,
} from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { ViewHeader } from "@/components/ViewHeader";
import { useStableCallback } from "@/hooks/useStableCallback";
import { useT } from "@/i18n";
import { copySecret } from "@/lib/clipboard";
import { api } from "@/lib/ipc";
import { taskActive } from "@/lib/tasks";
import { AccountActionsDialog } from "./AccountActionsDialog";
import { restoreAction, type RestoreDestination } from "./accountActions";
import { AccountsTable, type RowAction } from "./AccountsTable";
import { BulkProgress } from "./BulkProgress";
import { useClipboardFormats } from "./clipboardFormats";
import { LaunchControl } from "./launch/LaunchControl";
import { LaunchPanel } from "./launch/LaunchPanel";
import { useLauncher } from "./launch/useLauncher";
import type { RunAction } from "@/lib/actions";
import { useQuickActions, type QuickActionId } from "./quickActions";
import { RemoveAccountsDialog, type Removal } from "./RemoveAccountsDialog";
import { SelectionBar } from "./SelectionBar";
import { SessionFixBanner } from "./SessionFixBanner";
import { useAccountView, type FolderFilter } from "./useAccountView";
import { usePrivacy } from "@/lib/privacy";
import {
  BefriendDialog,
  PrivacyBulkDialog,
  type BulkTool,
} from "./BulkRobloxDialogs";
import { moveInOrder } from "./accountDetails";

const HEALTH_FILTERS: AccountHealth[] = [
  "unchecked",
  "healthy",
  "captcha_required",
  "invalid_session",
  "error",
];

/** The Accounts view: the account table with its filters, selection actions and launching. */
export function AccountsWorkspace({
  snapshot,
  selectedFolder,
  health,
  onHealth,
  trash,
  loading,
  onAdd,
  onEdit,
  onBulkUpdate,
  onConfig,
  run,
}: {
  snapshot: AppSnapshot;
  selectedFolder: FolderFilter;
  health: string;
  onHealth: (health: string) => void;
  trash: boolean;
  loading: boolean;
  onAdd: () => void;
  onEdit: (account: Account) => void;
  onBulkUpdate: () => void;
  onConfig: (patch: Partial<AppConfig>) => void;
  run: RunAction;
}) {
  const t = useT();
  const config = snapshot.config;
  const view = useAccountView(
    snapshot.accounts,
    selectedFolder,
    trash,
    config.searchSeparators,
    health,
    onHealth,
  );
  const ids = view.selected;
  const selectedAccounts = useMemo(() => {
    const chosen = new Set(ids);
    return snapshot.accounts.filter((account) => chosen.has(account.id));
  }, [ids, snapshot.accounts]);
  const launcher = useLauncher(config, run);
  const clipboard = useClipboardFormats();
  const { actions: quickActions } = useQuickActions();
  const [removal, setRemoval] = useState<Removal | null>(null);
  const [rowActions, setRowActions] = useState<Account | null>(null);
  const [bulk, setBulk] = useState<BulkTool | null>(null);
  const sessionsReady = snapshot.runtime.capabilities.includes("sessions");
  const canLaunch = snapshot.runtime.capabilities.includes("launch");
  // A manual browser waiting for the user keeps running after Stop.
  const stoppable = snapshot.tasks.some(
    (task) =>
      taskActive(task) &&
      !(task.kind === "open_browser" && task.status === "waiting_user"),
  );
  const privacy = usePrivacy();
  const accountNames = useMemo(
    () =>
      new Map(
        snapshot.accounts.map((account) => [
          account.id,
          privacy.username(account),
        ]),
      ),
    [snapshot.accounts, privacy],
  );
  const folderName =
    selectedFolder === "all"
      ? t("accounts.folders.all")
      : (snapshot.folders.find((folder) => folder.id === selectedFolder)
          ?.name ?? t("accounts.folders.general"));

  const copy = (accountIds: number[]) =>
    run(
      async () =>
        copySecret(
          await api.copyAccounts(accountIds, clipboard.selected.template),
        ),
      t("accounts.copied", { count: accountIds.length }),
    );
  const accountAction = (
    accountIds: number[],
    action: string,
    folderId: number | null,
    success: string,
  ) =>
    run(async () => {
      await api.accountAction(accountIds, action, folderId);
      view.clearSelection();
    }, success);
  const restore = (accountIds: number[], destination: RestoreDestination) =>
    accountAction(
      accountIds,
      ...restoreAction(destination),
      t("accounts.restored", { count: accountIds.length }),
    );
  // Row actions and the row menu act on one account; removal still asks first.
  const askRemove = (account: Account, kind: Removal) => {
    view.selectOnly(account.id);
    setRemoval(kind);
  };

  // Trash only offers restore and delete, which live in the row menu.
  // Memoized: the table rebuilds its columns when these change.
  const quickRowActions = useMemo<RowAction[]>(
    () =>
      trash
        ? []
        : quickActions.map((action) => ({
            ...action,
            disabled:
              action.id === "launch"
                ? !canLaunch
                : action.id !== "edit" &&
                  action.id !== "trash" &&
                  !sessionsReady,
          })),
    [trash, quickActions, canLaunch, sessionsReady],
  );
  const onRowAction = useStableCallback(
    (id: QuickActionId, account: Account) => {
      if (id === "launch") launcher.launch([account.id]);
      else if (id === "edit") onEdit(account);
      else if (id === "trash") askRemove(account, "trash");
      else run(() => api.startSessionTasks([account.id], id));
    },
  );

  return (
    <>
      <ViewHeader
        icon={trash ? Trash2 : Users}
        title={t(trash ? "accounts.trashTitle" : "accounts.title")}
        subtitle={
          <>
            {t("accounts.count", { count: view.inScope.length })}
            <span>•</span>
            {trash ? t("accounts.trashSubtitle") : folderName}
            <span>•</span>
            {t("accounts.selection.count", { count: ids.length })}
          </>
        }
        actions={
          !trash && (
            <>
              <Button
                variant="outline"
                disabled={!ids.length || !sessionsReady}
                onClick={() =>
                  run(() => api.startSessionTasks(ids, "check_health"))
                }
              >
                <ShieldCheck />
                {t("accounts.checkHealth")}
              </Button>
              <Button
                variant="outline"
                disabled={!stoppable}
                onClick={() => run(api.stopTasks)}
              >
                <Square />
                {t("accounts.stop")}
              </Button>
              <LaunchControl
                config={config}
                launcher={launcher}
                selected={ids}
                canLaunch={canLaunch}
              />
            </>
          )
        }
      />
      <div className="account-manager-layout">
        <div className="accounts-content">
          {!trash && (
            <BulkProgress tasks={snapshot.tasks} accountNames={accountNames} />
          )}
          {!trash && (
            <SessionFixBanner
              accounts={snapshot.accounts}
              disabled={!sessionsReady}
              onFix={(expired, stale) =>
                run(
                  async () => {
                    if (expired.length)
                      await api.startSessionTasks(expired, "refresh_cookie");
                    if (stale.length)
                      await api.startSessionTasks(stale, "check_health");
                  },
                  t("accounts.fix.started", {
                    count: expired.length + stale.length,
                  }),
                )
              }
            />
          )}
          <section className="account-section">
            <div className="account-section-header">
              <div className="table-tools table-filters">
                <label className="search-field accounts-search">
                  <Search size={13} />
                  <input
                    aria-label={t("accounts.search")}
                    placeholder={t("accounts.searchPlaceholder")}
                    value={view.search}
                    onChange={(event) => view.setSearch(event.target.value)}
                  />
                  {view.search && (
                    <button
                      aria-label={t("accounts.clearSearch")}
                      onClick={() => view.setSearch("")}
                    >
                      <X size={12} />
                    </button>
                  )}
                </label>
                <select
                  aria-label={t("accounts.statusFilter")}
                  value={view.health}
                  onChange={(event) => view.setHealth(event.target.value)}
                >
                  <option value="all">{t("accounts.allStatuses")}</option>
                  {HEALTH_FILTERS.map((health) => (
                    <option key={health} value={health}>
                      {t(`health.${health}`)}
                    </option>
                  ))}
                </select>
              </div>
              {!trash && (
                <div className="table-tools">
                  <Button
                    variant="outline"
                    size="icon"
                    disabled={!sessionsReady}
                    aria-label={t("accounts.refreshPresence")}
                    title={t("accounts.refreshPresence")}
                    onClick={() =>
                      run(api.refreshPresence, (count) =>
                        t("accounts.presenceRefreshed", { count }),
                      )
                    }
                  >
                    <Radio />
                  </Button>
                  <Button variant="outline" onClick={onBulkUpdate}>
                    <RefreshCw />
                    {t("accounts.update")}
                  </Button>
                  <Button onClick={onAdd}>
                    <Plus />
                    {t("accounts.add")}
                  </Button>
                </div>
              )}
            </div>
            {ids.length > 0 && (
              <SelectionBar
                count={ids.length}
                trash={trash}
                folders={snapshot.folders}
                sessionsReady={sessionsReady}
                onSession={(kind) =>
                  run(() => api.startSessionTasks(ids, kind))
                }
                onMove={(folderId) =>
                  accountAction(
                    ids,
                    "move",
                    folderId,
                    t("accounts.moved", { count: ids.length }),
                  )
                }
                onRobux={() =>
                  run(
                    () => api.refreshRobux(ids),
                    (count) => t("accounts.robux.checked", { count }),
                  )
                }
                onRoblox={setBulk}
                onCopy={() => copy(ids)}
                onExport={(format) =>
                  run(
                    () => api.exportAccounts(ids, format),
                    (saved) =>
                      saved
                        ? t("accounts.export.done", { count: ids.length })
                        : "",
                  )
                }
                onRestore={(destination) => restore(ids, destination)}
                onRemove={() => setRemoval(trash ? "purge" : "trash")}
                onClear={view.clearSelection}
              />
            )}
            <AccountsTable
              search={view.search}
              separators={config.searchSeparators}
              accounts={view.shown}
              tasks={snapshot.tasks}
              folders={snapshot.folders}
              selection={view.selection}
              onSelection={view.setSelection}
              onEdit={onEdit}
              onActions={setRowActions}
              rowActions={quickRowActions}
              onRowAction={onRowAction}
              onOrder={view.setOrderedIds}
              onReorder={
                trash
                  ? undefined
                  : (moved, target) => {
                      const order = moveInOrder(
                        snapshot.accounts.filter(
                          (account) => !account.deletedAt,
                        ),
                        moved,
                        target,
                      );
                      if (order) run(() => api.reorderAccounts(order));
                    }
              }
              onAdd={onAdd}
              trash={trash}
              loading={loading}
              compact={config.compactList}
            />
          </section>
        </div>
        {!trash && launcher.open && (
          <LaunchPanel
            config={config}
            launcher={snapshot.launcher}
            accountId={ids[0] ?? null}
            accountName={
              ids[0] === undefined ? null : (accountNames.get(ids[0]) ?? null)
            }
            selectedCount={ids.length}
            privateServer={launcher.privateServer}
            notice={launcher.notice}
            onPrivateServer={launcher.setPrivateServer}
            onConfig={onConfig}
            onLaunch={() => {
              launcher.close();
              launcher.launch(ids);
            }}
            onClose={launcher.close}
          />
        )}
      </div>
      <AccountActionsDialog
        account={rowActions}
        folders={snapshot.folders}
        trash={trash}
        canLaunch={canLaunch}
        sessionsReady={sessionsReady}
        onClose={() => setRowActions(null)}
        onEdit={onEdit}
        onLaunch={(account) => launcher.launch([account.id])}
        onSession={(account, kind) =>
          run(() => api.startSessionTasks([account.id], kind))
        }
        onCopy={(account) => copy([account.id])}
        onTrash={(account) => askRemove(account, trash ? "purge" : "trash")}
        onRestore={(account, destination) => restore([account.id], destination)}
      />
      {bulk === "privacy" && (
        <PrivacyBulkDialog
          accounts={selectedAccounts}
          onClose={() => setBulk(null)}
          onApply={(key, value) =>
            run(
              () => api.setPrivacy(ids, key, value),
              (count) =>
                t("accounts.bulk.privacy.done", {
                  count,
                  total: ids.length,
                }),
            )
          }
        />
      )}
      {bulk === "befriend" && (
        <BefriendDialog
          accounts={selectedAccounts}
          onClose={() => setBulk(null)}
          onApply={(mode) =>
            run(
              () => api.befriendAccounts(ids, mode),
              (summary) => t("accounts.bulk.befriend.done", { ...summary }),
            )
          }
        />
      )}
      <RemoveAccountsDialog
        removal={removal}
        count={ids.length}
        onConfirm={(kind) =>
          accountAction(
            ids,
            kind,
            null,
            t("accounts.removed", { count: ids.length }),
          )
        }
        onClose={() => setRemoval(null)}
      />
    </>
  );
}
