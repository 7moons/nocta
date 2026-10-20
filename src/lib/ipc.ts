import { invoke, isTauri, Channel } from "@tauri-apps/api/core";
import { t } from "@/i18n";
import type {
  AccountUpdate,
  AppConfig,
  AppSnapshot,
  FolderInput,
  ImportReport,
  ImportProgress,
  TaskKind,
  LaunchTargetInput,
  LaunchTargetSummary,
  LatestClient,
  LegacySource,
  ServerListOptions,
  ServerListResponse,
  PlaceInfo,
  GameInstance,
  GameSearchResult,
  AccountDetails,
  TotpCode,
  ExportFormat,
  FriendEntry,
  RobloxUser,
  AllowedFlag,
  BefriendMode,
  BefriendSummary,
  PrivacySetting,
  QuickLoginDevice,
  SystemMemory,
} from "@/generated/contracts";

export const desktop = isTauri();
function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!desktop) return Promise.reject(new Error(t("errors.previewReadOnly")));
  return invoke<T>(command, args);
}

export const api = {
  launchGame: (ids: number[], target: LaunchTargetInput) =>
    call<number[]>("launch_game", { ids, target }),
  resolveLaunchTarget: (accountId: number | null, target: LaunchTargetInput) =>
    call<LaunchTargetSummary>("resolve_launch_target", { accountId, target }),
  listServers: (
    requestKey: string,
    placeId: string,
    accountId: number | null,
    includeVip: boolean,
    options: ServerListOptions = {
      mostPlayersFirst: false,
      excludeFull: false,
      cursor: null,
    },
  ) =>
    call<ServerListResponse>("list_servers", {
      requestKey,
      placeId,
      accountId,
      includeVip,
      options,
    }),
  cancelServerList: (requestKey: string) =>
    call<void>("cancel_server_list", { requestKey }),
  placeInfo: (placeId: string) =>
    call<PlaceInfo>("get_place_info", { placeId }),
  expandInviteLink: (url: string) =>
    call<string>("expand_invite_link", { url }),
  searchGames: (query: string) =>
    call<GameSearchResult[]>("search_games", { query }),
  instances: () => call<GameInstance[]>("list_instances"),
  closeInstance: (id: string) => call<void>("close_instance", { id }),
  closeAllInstances: () => call<number>("close_all_owned_instances"),
  closeExternalInstance: (id: string) =>
    call<void>("close_external_instance", { id }),
  closeAllRoblox: () => call<number>("close_all_roblox"),
  clientCheckLatest: () => call<LatestClient>("client_check_latest"),
  clientInstall: (guid: string | null = null) =>
    call<void>("client_install", { guid }),
  clientCancelInstall: () => call<void>("client_cancel_install"),
  clientRemove: (guid: string) => call<void>("client_remove", { guid }),
  clientCleanup: () => call<number>("client_cleanup"),
  startSessionTasks: (ids: number[], kind: TaskKind) =>
    call<number[]>("start_session_tasks", { ids, kind }),
  stopTasks: () => call<void>("stop_tasks"),
  cancelTask: (id: number) => call<void>("cancel_task", { id }),
  snapshot: () => call<AppSnapshot>("get_snapshot"),
  importAccounts: (
    text: string,
    folderId: number | null,
    updateExisting: boolean,
    onProgress?: (progress: ImportProgress) => void,
  ) => {
    if (!desktop) return call<ImportReport>("import_accounts");
    const progress = new Channel<ImportProgress>();
    progress.onmessage = (value) => onProgress?.(value);
    return call<ImportReport>("import_accounts", {
      text,
      folderId,
      updateExisting,
      progress,
    });
  },
  legacyDetect: () => call<LegacySource[]>("legacy_detect"),
  legacyPick: () => call<LegacySource | null>("legacy_pick"),
  legacyImport: (
    path: string,
    includeTrash: boolean,
    onProgress?: (progress: ImportProgress) => void,
    password: string | null = null,
  ) => {
    if (!desktop) return call<ImportReport>("legacy_import");
    const progress = new Channel<ImportProgress>();
    progress.onmessage = (value) => onProgress?.(value);
    return call<ImportReport>("legacy_import", {
      path,
      includeTrash,
      password,
      progress,
    });
  },
  updateAccount: (id: number, input: AccountUpdate) =>
    call<void>("update_account", { id, input, validateSession: true }),
  accountAction: (
    ids: number[],
    action: string,
    folderId: number | null = null,
  ) => call<void>("account_action", { ids, action, folderId }),
  saveFolder: (id: number | null, input: FolderInput) =>
    call<void>("save_folder", { id, input }),
  deleteFolder: (id: number) => call<void>("delete_folder", { id }),
  moveFolder: (id: number, group: string | null, before: number | null) =>
    call<void>("move_folder", { id, group, before }),
  renameFolderGroup: (from: string, to: string | null) =>
    call<void>("rename_folder_group", { from, to }),
  patchConfig: (patch: Partial<AppConfig>) =>
    call<void>("patch_config", { patch }),
  copyAccounts: (ids: number[], format: string) =>
    call<string>("copy_accounts", { ids, format }),
  exportBackup: (password: string) =>
    call<boolean>("export_backup", { password }),
  restoreBackup: (password: string) =>
    call<number | null>("restore_backup", { password }),
  updateAccountDetails: (id: number, details: AccountDetails) =>
    call<void>("update_account_details", { id, details }),
  setTotp: (id: number, key: string | null) =>
    call<void>("set_totp", { id, key }),
  totpCode: (id: number) => call<TotpCode>("totp_code", { id }),
  exportAccounts: (ids: number[], format: ExportFormat) =>
    call<boolean>("export_accounts", { ids, format }),
  signInAccount: (folderId: number | null) =>
    call<number>("sign_in_account", { folderId }),
  friendsOf: (accountId: number) =>
    call<FriendEntry[]>("friends_of", { accountId }),
  refreshPresence: () => call<number>("refresh_presence"),
  userAvatars: (userIds: string[]) =>
    call<Record<string, string>>("user_avatars", { userIds }),
  findUser: (username: string) => call<RobloxUser>("find_user", { username }),
  runPreset: (id: string) => call<number[]>("run_preset", { id }),
  testWebhook: (url: string) => call<void>("test_webhook", { url }),
  refreshRobux: (ids: number[]) => call<number>("refresh_robux", { ids }),
  allowedFastFlags: () => call<AllowedFlag[]>("allowed_fast_flags"),
  arrangeWindows: () => call<number>("arrange_windows"),
  systemMemory: () => call<SystemMemory | null>("system_memory"),
  uiReady: () => call<void>("ui_ready"),
  openHomepage: () => call<void>("open_homepage"),
  setSafeDisplay: (on: boolean) => call<void>("set_safe_display", { on }),
  reorderAccounts: (ids: number[]) => call<void>("reorder_accounts", { ids }),
  signOutOtherSessions: (id: number) =>
    call<void>("sign_out_other_sessions", { id }),
  privacySettings: (id: number) =>
    call<PrivacySetting[]>("privacy_settings", { id }),
  setPrivacy: (ids: number[], key: string, value: string) =>
    call<number>("set_privacy", { ids, key, value }),
  setDisplayName: (id: number, name: string) =>
    call<void>("set_display_name", { id, name }),
  quickLoginCheck: (id: number, code: string) =>
    call<QuickLoginDevice>("quick_login_check", { id, code }),
  quickLoginApprove: (id: number, code: string) =>
    call<void>("quick_login_approve", { id, code }),
  befriendAccounts: (ids: number[], mode: BefriendMode) =>
    call<BefriendSummary>("befriend_accounts", { ids, mode }),
};
