// Generated from nocta-core. Run bun run contracts; do not edit manually.

export type AccountHealth =
  "unchecked" | "healthy" | "captcha_required" | "invalid_session" | "error";

export type AccountPresence = {
  userPresenceType: number;
  lastLocation: string | null;
  lastOnline: string | null;
  placeId: string | null;
};

export type Account = {
  id: number;
  username: string;
  folderId: number | null;
  health: AccountHealth;
  checkedAt: string | null;
  createdAt: string;
  deletedAt: string | null;
  hasPassword: boolean;
  hasCookie: boolean;
  userId: string | null;
  cookieValid: boolean | null;
  lastStartedAt: string | null;
  presence: AccountPresence | null;
  lastTask: AccountTask | null;
  /**
   * The user's own name for the account, shown next to the username.
   */
  alias: string;
  note: string;
  /**
   * Color tag, `#rrggbb`.
   */
  color: string | null;
  /**
   * Pinned accounts are listed first.
   */
  pinned: boolean;
  /**
   * Whether an authenticator key is saved for two-step verification codes.
   */
  hasTotp: boolean;
  /**
   * Time spent in games launched by Nocta.
   */
  playtimeSecs: number;
  /**
   * Robux balance when last checked.
   */
  robux: number | null;
  robuxCheckedAt: string | null;
  /**
   * Main accounts keep running; periodic restarts and the alt profile skip them.
   */
  isMain: boolean;
  /**
   * Position in the user's own order (dragged in the list); `None` comes after, by ID.
   */
  sortOrder: number | null;
  /**
   * The user's own fields, like "Server: EU".
   */
  fields: Array<AccountField>;
  /**
   * Where anti-AFK clicks in this account's client, instead of the general action.
   */
  afkPoint: ClickPoint | null;
};

export type AccountDetails = {
  alias: string;
  note: string;
  color: string | null;
  pinned: boolean;
  isMain: boolean;
  fields: Array<AccountField>;
  afkPoint: ClickPoint | null;
};

export type TotpCode = { code: string; remainingSecs: number };

export type Folder = {
  id: number;
  name: string;
  color: string;
  group: string | null;
  sortOrder: number;
};

export type DurationSummary = {
  samples: number;
  averageMs: number;
  p95Ms: number;
};

export type MetricErrors = {
  checkTimeout: number;
  verifyTimeout: number;
  ambiguousPrecheck: number;
};

export type MetricsSummary = {
  checksTotal: number;
  captchasDetectedTotal: number;
  launchesTotal: number;
  launchesSuccessTotal: number;
  check: DurationSummary;
  launch: DurationSummary;
  errors: MetricErrors;
};

export type LaunchMode = "public" | "job" | "private";

export type JoinStrategy =
  "squad" | "lowest_ping" | "hop" | "fullest" | "emptiest";

export type PresetTarget =
  | { kind: "place"; placeId: string }
  | { kind: "link"; url: string }
  | { kind: "pick"; placeId: string; strategy: JoinStrategy }
  | { kind: "follow"; username: string };

export type LaunchPreset = {
  id: string;
  name: string;
  accountIds: Array<number>;
  target: PresetTarget;
  /**
   * Launch with the AFK profile (frame-rate cap, lowest graphics, minimized windows).
   */
  afk: boolean;
  /**
   * System-wide shortcut that runs the preset; empty means none.
   */
  hotkey: string;
};

export type Schedule = {
  id: string;
  presetId: string;
  /**
   * Weekdays, 0 = Monday … 6 = Sunday.
   */
  days: Array<number>;
  /**
   * Local time, `HH:MM`.
   */
  time: string;
  /**
   * Close the preset's clients this many minutes after launching; 0 keeps them running.
   */
  closeAfterMinutes: number;
  enabled: boolean;
};

export type AntiAfkAction = "turn" | "jump" | "click";

export type LaunchPacing = "in_game" | "fixed";

export type ArrangeMonitors = "all" | "primary";

export type ProcessPriority = "normal" | "below_normal" | "idle";

export type ClientProfile = {
  priority: ProcessPriority;
  /**
   * Windows' efficiency mode (EcoQoS): lower clocks and efficient cores.
   */
  ecoMode: boolean;
  /**
   * Share of all CPU cores the client may use, in percent; 0 is unlimited.
   */
  cpuLimitPercent: number;
  /**
   * Window size in pixels; 0 leaves the size Roblox chooses.
   */
  windowWidth: number;
  windowHeight: number;
  /**
   * Windows takes memory from these clients first when it runs short.
   */
  lowMemoryPriority: boolean;
  /**
   * Gives back the client's unused memory this often, in seconds; 0 is off. A client in a
   * game touches only part of what it holds: measured, a client at 1.7 GB of RAM went down to
   * 0.1 GB and settled near 0.8 GB.
   */
  trimEverySecs: number;
  /**
   * Most RAM the client may hold, in megabytes; 0 is no cap. Windows keeps the rest of what
   * it allocated compressed or in the page file. Measured in a game: 939 MB without a cap,
   * 350 MB with one, for 0.3% more CPU; at 250 MB reloading pages starts to cost CPU.
   */
  ramCapMb: number;
};

export type ClientStage = "loading" | "joining" | "in_game" | "disconnected";

export type ClickPoint = { xPercent: number; yPercent: number };

export type SystemMemory = {
  ramTotalMb: number;
  ramFreeMb: number;
  commitLimitMb: number;
  commitUsedMb: number;
};

export type ServerRegion =
  "north_america" | "south_america" | "europe" | "asia" | "oceania" | "africa";

export type AccountField = { name: string; value: string };

export type PrivacySetting = {
  /**
   * Roblox's name, like `whoCanJoinMeInExperiences`.
   */
  key: string;
  value: string;
  options: Array<string>;
  /**
   * Options Roblox only allows after a check of its own (like an age check), with what it
   * wants. They cannot be chosen from Nocta, so leaving one of them is one-way.
   */
  restricted: Array<RestrictedOption>;
};

export type RestrictedOption = {
  value: string;
  /**
   * Roblox's name for the check, like `FacialAgeEstimation`.
   */
  requirement: string;
};

export type QuickLoginDevice = { device: string; location: string };

export type BefriendMode =
  { kind: "everyone" } | { kind: "with_main"; main: number };

export type BefriendSummary = {
  pairs: number;
  /**
   * Became friends now.
   */
  added: number;
  already: number;
  /**
   * Sent, but Roblox wants a captcha or the other side could not accept.
   */
  pending: number;
  failed: number;
};

export type Hotkeys = { show: string; closeRoblox: string; toggleAfk: string };

export type FastFlag = { name: string; value: string };

export type FlagKind = "bool" | "int";

export type AllowedFlag = { name: string; kind: FlagKind };

export type ExportFormat = "csv" | "json";

export type LaunchTargetInput =
  | { kind: "configured" }
  | { kind: "public"; placeId: string }
  | { kind: "job"; placeId: string; jobId: string }
  | {
      kind: "private";
      placeId: string;
      accessCode: string;
      linkCode: string | null;
    }
  | { kind: "server"; targetId: string }
  | { kind: "invite"; url: string }
  | {
      kind: "pick";
      placeId: string;
      strategy: JoinStrategy;
      excludeJob: string | null;
    }
  | { kind: "follow"; username: string };

export type LaunchTargetSummary = {
  mode: LaunchMode;
  placeId: string;
  jobId: string | null;
  usedFallback: boolean;
  /**
   * Opaque backend reference for private targets; never contains the access code.
   */
  targetId: string | null;
};

export type ServerKind = "public" | "vip";

export type ServerRow = {
  id: string;
  kind: ServerKind;
  playing: number;
  maxPlayers: number;
  playerTokens: Array<string>;
  name: string | null;
  targetId: string | null;
  joinable: boolean;
  /**
   * Server ping in ms as reported by Roblox (public servers only).
   */
  ping: number | null;
  /**
   * Server frame rate as reported by Roblox (public servers only).
   */
  fps: number | null;
};

export type ServerListResponse = {
  placeId: string;
  items: Array<ServerRow>;
  warnings: Array<string>;
  cancelled: boolean;
  /**
   * Cursor for the next page of public servers; pass it back to load more.
   */
  nextCursor: string | null;
};

export type ServerListOptions = {
  mostPlayersFirst: boolean;
  excludeFull: boolean;
  cursor: string | null;
};

export type PlaceInfo = {
  placeId: string;
  name: string;
  universeId: string | null;
  iconUrl: string | null;
};

export type GameSearchResult = {
  placeId: string;
  universeId: string;
  name: string;
  creator: string | null;
  playing: number;
  iconUrl: string | null;
};

export type RobloxUser = { id: string; name: string; displayName: string };

export type UserPresence = {
  userId: string;
  kind: number;
  placeId: string | null;
  jobId: string | null;
  location: string | null;
};

export type FriendEntry = {
  user: RobloxUser;
  presence: UserPresence | null;
  avatarUrl: string | null;
};

export type InstanceStatus = "starting" | "running" | "exited" | "unknown";

export type GameInstance = {
  id: string;
  accountId: number;
  username: string;
  placeId: string;
  mode: LaunchMode;
  status: InstanceStatus;
  pid: number | null;
  processName: string | null;
  startedAt: string;
  exitedAt: string | null;
  detail: string;
  /**
   * What the client's own log says: still loading, joining, in its game, or disconnected.
   */
  stage: ClientStage;
  /**
   * Why Roblox disconnected the client, when it did.
   */
  disconnect: string | null;
  /**
   * Memory the client holds right now (its working set), in megabytes.
   */
  memoryMb: number | null;
};

export type ExternalInstance = {
  /**
   * Identity token `pid:creation-ms`; closing re-verifies it so a reused PID is never targeted.
   */
  id: string;
  pid: number;
  processName: string;
  startedAt: string;
};

export type MultiInstanceState =
  "disabled" | "acquired" | "abandoned" | "busy" | "unsupported" | "failed";

export type LauncherState = {
  handlerRegistered: boolean;
  handlerPath: string | null;
  multiInstance: MultiInstanceState;
  multiInstanceSupported: boolean;
  detail: string | null;
};

export type GamePreset = { id: string; name: string; placeId: string };

export type LogPreferences = {
  level: string;
  module: string;
  showTimestamp: boolean;
  showAccount: boolean;
  showModule: boolean;
  showLevel: boolean;
};

export type ClientUpdateMode = "auto" | "ask" | "manual";

export type ClientGraphicsApi = "auto" | "d3d11" | "vulkan" | "opengl";

export type InstalledClient = {
  guid: string;
  /**
   * Display version such as 0.741.0.7411058, when known.
   */
  version: string | null;
  installedAt: string;
  sizeMb: number;
};

export type LatestClient = { guid: string; version: string; checkedAt: string };

export type ClientInstallProgress = {
  guid: string;
  /**
   * "preparing", "downloading", "extracting" or "finishing".
   */
  phase: string;
  doneMb: number;
  totalMb: number;
  packagesDone: number;
  packagesTotal: number;
};

export type ClientState = {
  installed: Array<InstalledClient>;
  /**
   * GUID used for the next launch.
   */
  active: string | null;
  latest: LatestClient | null;
  progress: ClientInstallProgress | null;
  lastError: string | null;
  directory: string;
};

export type AppConfig = {
  checkerConcurrency: number;
  precheckRetries: number;
  browserTimeoutSecs: number;
  checkerDelaySecs: number;
  bulkActionDelaySecs: number;
  browserAppMode: boolean;
  launchPrecheck: boolean;
  preLaunchDelaySecs: number;
  multiInstance: boolean;
  autoFixPrivateInvitePrivacy: boolean;
  placeId: string;
  inviteUrl: string;
  presets: Array<GamePreset>;
  alwaysOnTop: boolean;
  privacyMode: boolean;
  interfaceScale: number;
  showStatusBar: boolean;
  defaultFolderColor: string;
  searchSeparators: string;
  logPreferences: LogPreferences;
  /**
   * Launch accounts with Nocta's own Roblox client instead of the system `roblox-player` handler.
   */
  managedClient: boolean;
  clientUpdateMode: ClientUpdateMode;
  /**
   * Previous client versions kept for rollback, besides the active one.
   */
  clientKeepVersions: number;
  /**
   * Version GUID to launch with; `None` launches the newest installed version.
   */
  clientPinnedVersion: string | null;
  /**
   * Lowest rendering, texture and detail quality for Nocta's client (allowlisted flags only).
   */
  clientLowGraphics: boolean;
  clientGraphicsApi: ClientGraphicsApi;
  /**
   * Roblox's frame-rate cap written before launches; 0 leaves Roblox's own setting.
   */
  fpsCap: number;
  /**
   * AFK profile: its frame-rate cap, lowest graphics and minimized windows.
   */
  afkMode: boolean;
  afkFpsCap: number;
  afkMinimize: boolean;
  /**
   * Relaunch a client into the same server when it closes on its own.
   */
  autoRejoin: boolean;
  /**
   * Discord webhook told about crashes and rejoins; empty for none.
   */
  discordWebhook: string;
  /**
   * How the launch panel picks a server; `None` is any server or the saved link.
   */
  joinStrategy: JoinStrategy | null;
  /**
   * Player followed when launching with "follow a player".
   */
  followUsername: string;
  launchPresets: Array<LaunchPreset>;
  schedules: Array<Schedule>;
  /**
   * Press a key in idle clients now and then so Roblox does not kick them after 20 minutes.
   */
  antiAfk: boolean;
  antiAfkMinutes: number;
  /**
   * Vary each client's interval a little, so they do not all act at the same second.
   */
  antiAfkRandom: boolean;
  antiAfkAction: AntiAfkAction;
  /**
   * Page out the clients' unused memory every few minutes.
   */
  trimMemory: boolean;
  /**
   * Close (and with auto-rejoin, restart) a client using more memory than this; 0 is off.
   */
  memoryLimitMb: number;
  /**
   * The window's close button hides Nocta in the tray instead of quitting.
   */
  closeToTray: boolean;
  startWithWindows: boolean;
  /**
   * Started with Windows: stay in the tray instead of opening the window.
   */
  startMinimized: boolean;
  hotkeys: Hotkeys;
  /**
   * Roblox FastFlags for Nocta's own client, limited to the flags Roblox allows.
   */
  fastFlags: Array<FastFlag>;
  /**
   * Shorter account rows with one line each, so more fit on screen.
   */
  compactList: boolean;
  /**
   * When the next account of a launch starts.
   */
  launchPacing: LaunchPacing;
  /**
   * With `InGame` pacing, the longest wait for a client to join before the next one starts.
   */
  inGameTimeoutSecs: number;
  /**
   * Title each client window with its account's name.
   */
  nameWindows: boolean;
  arrangeMonitors: ArrangeMonitors;
  /**
   * Arrange the windows again whenever a client appears.
   */
  autoArrange: boolean;
  /**
   * Close (and with auto-rejoin, restart) clients whose window stopped responding.
   */
  closeHung: boolean;
  /**
   * Close (and with auto-rejoin, restart) clients that Roblox disconnected.
   */
  closeDisconnected: boolean;
  /**
   * Restart clients of accounts not marked as main every so many minutes; 0 is off.
   */
  restartAltsMinutes: number;
  /**
   * Resource profile for clients of main accounts.
   */
  mainProfile: ClientProfile;
  /**
   * Resource profile for every other client.
   */
  altProfile: ClientProfile;
  /**
   * Keep the computer awake while clients run with anti-AFK, auto-rejoin or schedules.
   */
  keepAwake: boolean;
  /**
   * Dragging a Roblox window's border goes below Roblox's minimum (800×600), down to 1×1.
   */
  freeWindowResize: boolean;
  /**
   * Colors the user saved for folders, `#rrggbb`, shown next to the built-in ones.
   */
  folderColors: Array<string>;
  /**
   * Where anti-AFK clicks when its action is `Click` (accounts can have their own point).
   */
  antiAfkClick: ClickPoint;
  /**
   * Hide account names (aliases stay) so the screen can be shared or streamed.
   */
  streamerMode: boolean;
  /**
   * Picked servers (squad, fullest, hop…) must be in this part of the world.
   */
  serverRegion: ServerRegion | null;
};

export type LogEntry = {
  id: number;
  timestamp: string;
  level: string;
  module: string;
  message: string;
  accountId: number | null;
};

export type RuntimeInfo = {
  platform: string;
  version: string;
  dataDirectory: string;
  portable: boolean;
  browserReady: boolean;
  browserPath: string | null;
  capabilities: Array<string>;
  /**
   * The window is drawn without GPU acceleration (safe display mode).
   */
  safeDisplay: boolean;
};

export type TaskKind =
  | "check_health"
  | "validate_cookie"
  | "open_browser"
  | "refresh_cookie"
  | "launch_game";

export type TaskStatus =
  | "queued"
  | "running"
  | "waiting_user"
  | "cancelling"
  | "succeeded"
  | "failed"
  | "cancelled";

export type AccountTask = {
  id: number;
  accountId: number;
  kind: TaskKind;
  status: TaskStatus;
  detail: string;
  createdAt: string;
  finishedAt: string | null;
  durationMs: number | null;
};

export type AppSnapshot = {
  revision: number;
  accounts: Array<Account>;
  folders: Array<Folder>;
  config: AppConfig;
  logs: Array<LogEntry>;
  runtime: RuntimeInfo;
  tasks: Array<AccountTask>;
  metrics: MetricsSummary;
  instances: Array<GameInstance>;
  externalInstances: Array<ExternalInstance>;
  client: ClientState;
  launcher: LauncherState;
};

export type ImportIssue = { line: number; reason: string };

export type LegacySource = {
  path: string;
  app: string;
  accounts: number;
  inTrash: number;
  folders: number;
  modifiedAt: string | null;
  /**
   * The file is encrypted with the other app's password, which the import asks for.
   */
  needsPassword: boolean;
};

export type ImportReport = {
  added: number;
  updated: number;
  skipped: number;
  issues: Array<ImportIssue>;
};

export type ImportProgress = {
  processed: number;
  total: number;
  added: number;
  updated: number;
  failed: number;
  committed: boolean;
};

export type FolderInput = { name: string; color: string; group: string | null };

export type AccountUpdate = {
  username: string;
  folderId: number | null;
  password: string | null;
  cookie: string | null;
};

export type CommandError = { code: string; message: string };
