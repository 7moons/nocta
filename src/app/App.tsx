import { lazy, Suspense, useCallback, useEffect, useState } from "react";
import { AlertCircle, Settings2 } from "lucide-react";
import type { Account, AppConfig } from "@/generated/contracts";
import { ViewHeader } from "@/components/ViewHeader";
import { AccountsWorkspace } from "@/features/accounts/AccountsWorkspace";
import { EditAccountDialog } from "@/features/accounts/EditAccountDialog";
import { useSnapshot } from "@/hooks/useSnapshot";
import { useT } from "@/i18n";
import { api, desktop } from "@/lib/ipc";
import { PrivacyProvider } from "@/lib/privacy";
import { ActivityPanel } from "./ActivityPanel";
import { ClientUpdateBanner } from "./ClientUpdateBanner";
import {
  ChangelogDialog,
  useChangelog,
} from "@/features/changelog/ChangelogDialog";
import { CommandPalette } from "./CommandPalette";
import { ErrorBoundary } from "./ErrorBoundary";
import { MultiInstanceBusyDialog } from "./MultiInstanceBusyDialog";
import {
  NavigationProvider,
  useNavigationHistory,
  type View,
} from "./navigation";
import { Sidebar } from "./Sidebar";
import { StatusBar } from "./StatusBar";
import { TitleBar } from "./TitleBar";
import { Toast } from "./Toast";
import { useNotices } from "./useNotices";
import { useShortcuts } from "./useShortcuts";

// Views other than the accounts load when first opened, so their code is not in memory until
// then.
const ImportDialog = lazy(() =>
  import("@/features/accounts/ImportDialog").then((module) => ({
    default: module.ImportDialog,
  })),
);
const AutomationView = lazy(() =>
  import("@/features/automation/AutomationView").then((module) => ({
    default: module.AutomationView,
  })),
);
const DashboardView = lazy(() =>
  import("@/features/dashboard/DashboardView").then((module) => ({
    default: module.DashboardView,
  })),
);
const FriendsView = lazy(() =>
  import("@/features/friends/FriendsView").then((module) => ({
    default: module.FriendsView,
  })),
);
const SettingsView = lazy(() =>
  import("@/features/settings/SettingsView").then((module) => ({
    default: module.SettingsView,
  })),
);

// The interface scale setting is a percentage of this root font size.
const BASE_FONT_PX = 13;

export function App() {
  const t = useT();
  const { snapshot, loading, error, refresh } = useSnapshot();
  const history = useNavigationHistory({
    view: "accounts",
    folder: "all",
    health: "all",
  });
  const { view, folder: folderId, health } = history.current;
  const { navigate, replace, back, forward } = history;
  const setView = useCallback(
    (next: View) => navigate({ view: next }),
    [navigate],
  );
  const notices = useNotices();
  const { run } = notices;
  const changelog = useChangelog(loading ? null : snapshot.runtime.version);
  const [importMode, setImportMode] = useState<"add" | "update" | null>(null);
  const [editingAccount, setEditingAccount] = useState<Account | null>(null);
  useShortcuts(back, forward, navigate);

  // A deleted folder cannot stay selected.
  useEffect(() => {
    if (
      typeof folderId === "number" &&
      !snapshot.folders.some((folder) => folder.id === folderId)
    )
      replace({ folder: null });
  }, [folderId, snapshot.folders, replace]);
  // Tells the desktop shell the interface is up, so it does not fall back to safe display mode.
  useEffect(() => {
    if (!loading && desktop) api.uiReady().catch(() => undefined);
  }, [loading]);
  useEffect(() => {
    document.documentElement.style.fontSize = `${(BASE_FONT_PX * snapshot.config.interfaceScale) / 100}px`;
  }, [snapshot.config.interfaceScale]);

  const onConfig = useCallback(
    (patch: Partial<AppConfig>) =>
      run(() => api.patchConfig(patch), t("app.settingsSaved")),
    [run, t],
  );
  // Closes every Roblox client. With enableMulti, also turns multi-instance on when the
  // setting was rejected earlier because a running client held the lock.
  const closeAllRoblox = useCallback(
    (enableMulti: boolean) =>
      run(
        async () => {
          const closed = await api.closeAllRoblox();
          if (enableMulti && !snapshot.config.multiInstance)
            await api.patchConfig({ multiInstance: true });
          return closed;
        },
        (closed) =>
          t(enableMulti ? "app.robloxClosedMulti" : "app.robloxClosed", {
            count: closed,
          }),
      ),
    [snapshot.config.multiInstance, run, t],
  );

  const openImport = useCallback(() => setImportMode("add"), []);
  const runPreset = useCallback(
    (id: string, name: string) =>
      run(() => api.runPreset(id), t("automation.preset.started", { name })),
    [run, t],
  );
  const refreshPresence = useCallback(
    () =>
      run(api.refreshPresence, (count) =>
        t("accounts.presenceRefreshed", { count }),
      ),
    [run, t],
  );
  const arrangeWindows = useCallback(
    () => run(api.arrangeWindows, (count) => t("activity.arranged", { count })),
    [run, t],
  );
  const closeAllFromPalette = useCallback(
    () => closeAllRoblox(false),
    [closeAllRoblox],
  );

  return (
    <ErrorBoundary>
      <PrivacyProvider
        hidden={snapshot.config.streamerMode}
        accounts={snapshot.accounts}
      >
        <NavigationProvider back={back} canGoBack={history.canGoBack}>
          <div className="app-shell">
            <TitleBar />
            <div className="workspace">
              <Sidebar
                view={view}
                onView={setView}
                accounts={snapshot.accounts}
                folders={snapshot.folders}
                selectedFolder={folderId}
                onFolder={(folder) =>
                  navigate({ view: "accounts", folder, health: "all" })
                }
                palette={snapshot.config.folderColors}
                defaultFolderColor={snapshot.config.defaultFolderColor}
                onPalette={(folderColors) => onConfig({ folderColors })}
                run={run}
                onMoveAccounts={(ids, folder) =>
                  run(
                    () => api.accountAction(ids, "move", folder),
                    t("accounts.moved", { count: ids.length }),
                  )
                }
              />
              <main className="main-workspace">
                {error && (
                  <div className="error-banner">
                    <AlertCircle size={14} />
                    <span>{error}</span>
                    <button onClick={() => void refresh()}>
                      {t("app.retry")}
                    </button>
                  </div>
                )}
                <ClientUpdateBanner
                  config={snapshot.config}
                  client={snapshot.client}
                  onError={notices.showError}
                />
                <div className="main-view">
                  <Suspense fallback={null}>
                    {(view === "accounts" || view === "trash") && (
                      <AccountsWorkspace
                        snapshot={snapshot}
                        selectedFolder={folderId}
                        health={health}
                        onHealth={(value) => replace({ health: value })}
                        trash={view === "trash"}
                        loading={loading}
                        onAdd={() => setImportMode("add")}
                        onBulkUpdate={() => setImportMode("update")}
                        onEdit={setEditingAccount}
                        onConfig={onConfig}
                        run={run}
                      />
                    )}
                    {view === "settings" && (
                      <>
                        <ViewHeader
                          icon={Settings2}
                          title={t("settings.title")}
                          subtitle={t("settings.subtitle")}
                          actions={
                            notices.pending > 0 && (
                              <span className="badge">
                                {t("common.saving")}
                              </span>
                            )
                          }
                        />
                        <SettingsView
                          config={snapshot.config}
                          launcher={snapshot.launcher}
                          client={snapshot.client}
                          runtime={snapshot.runtime}
                          onChange={onConfig}
                        />
                      </>
                    )}
                    {view === "dashboard" && (
                      <DashboardView
                        snapshot={snapshot}
                        run={run}
                        onAccounts={(filter = "all") =>
                          navigate({
                            view: "accounts",
                            folder: "all",
                            health: filter,
                          })
                        }
                        onFolder={(folder) =>
                          navigate({ view: "accounts", folder, health: "all" })
                        }
                        onView={setView}
                        onAdd={() => setImportMode("add")}
                      />
                    )}
                    {view === "friends" && (
                      <FriendsView snapshot={snapshot} run={run} />
                    )}
                    {view === "automation" && (
                      <AutomationView
                        snapshot={snapshot}
                        onConfig={onConfig}
                        run={run}
                      />
                    )}
                  </Suspense>
                </div>
                <ActivityPanel
                  logs={snapshot.logs}
                  tasks={snapshot.tasks}
                  accounts={snapshot.accounts}
                  instances={snapshot.instances}
                  preferences={snapshot.config.logPreferences}
                  onPreferences={(logPreferences) =>
                    run(() => api.patchConfig({ logPreferences }))
                  }
                  externalInstances={snapshot.externalInstances}
                  onCloseInstance={(id) => run(() => api.closeInstance(id))}
                  onCloseExternal={(id) =>
                    run(
                      () => api.closeExternalInstance(id),
                      t("app.robloxClosed", { count: 1 }),
                    )
                  }
                  onCloseAll={() => run(api.closeAllInstances)}
                  onCloseAllRoblox={() => closeAllRoblox(false)}
                  onArrange={arrangeWindows}
                  onCancel={(id) => run(() => api.cancelTask(id))}
                />
              </main>
            </div>
            {snapshot.config.showStatusBar && (
              <StatusBar
                snapshot={snapshot}
                pending={notices.pending > 0}
                onChangelog={() => changelog.setOpen(true)}
              />
            )}
            {notices.notice && (
              <Toast notice={notices.notice} onDismiss={notices.dismiss} />
            )}
            <CommandPalette
              snapshot={snapshot}
              onView={setView}
              onAdd={openImport}
              onAccount={setEditingAccount}
              onRunPreset={runPreset}
              onConfig={onConfig}
              onRefreshPresence={refreshPresence}
              onCloseAllRoblox={closeAllFromPalette}
              onArrange={arrangeWindows}
              onChangelog={() => changelog.setOpen(true)}
            />
            <ChangelogDialog
              open={changelog.open}
              onOpenChange={changelog.setOpen}
            />
            <MultiInstanceBusyDialog
              snapshot={snapshot}
              onCloseAll={() => closeAllRoblox(true)}
            />
            {importMode !== null && (
              <Suspense fallback={null}>
                <ImportDialog
                  open={importMode !== null}
                  updateExisting={importMode === "update"}
                  onClose={() => setImportMode(null)}
                  folders={snapshot.folders}
                  folderId={typeof folderId === "number" ? folderId : null}
                  accounts={snapshot.accounts}
                />
              </Suspense>
            )}
            <EditAccountDialog
              account={editingAccount}
              folders={snapshot.folders}
              onClose={() => setEditingAccount(null)}
            />
          </div>
        </NavigationProvider>
      </PrivacyProvider>
    </ErrorBoundary>
  );
}
