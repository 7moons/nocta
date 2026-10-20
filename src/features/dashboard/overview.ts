import type {
  Account,
  AccountHealth,
  AppSnapshot,
  ClientStage,
  GameInstance,
} from "@/generated/contracts";
import { taskActive } from "@/lib/tasks";

/** Health states in display order, from good to bad; `unchecked` last. */
export const HEALTH_ORDER: AccountHealth[] = [
  "healthy",
  "captcha_required",
  "invalid_session",
  "error",
  "unchecked",
];
export const NEEDS_ATTENTION: AccountHealth[] = [
  "captcha_required",
  "invalid_session",
  "error",
];

const DAY_MS = 24 * 60 * 60 * 1000;

export type HealthCounts = Record<AccountHealth, number>;

export function healthCounts(accounts: Account[]): HealthCounts {
  const counts = Object.fromEntries(
    HEALTH_ORDER.map((health) => [health, 0]),
  ) as HealthCounts;
  for (const account of accounts) counts[account.health]++;
  return counts;
}

/** A share in whole percent, or null when there is nothing to divide. */
export const percent = (part: number, whole: number) =>
  whole > 0 ? Math.round((part / whole) * 100) : null;

/** One account that is playing: a client Nocta runs, or Roblox saying it is in a game. */
export interface PlayingRow {
  key: string;
  accountId: number;
  account: Account | null;
  username: string;
  placeId: string | null;
  /** Roblox's own name for where the account is, when only presence is known. */
  location: string | null;
  /** The client's stage; `presence` when Roblox reports the account in a game elsewhere. */
  stage: ClientStage | "presence";
  since: string | null;
  instance: GameInstance | null;
}

// Problems first, then those in a game, then the ones still getting there.
const STAGE_RANK: Record<PlayingRow["stage"], number> = {
  disconnected: 0,
  in_game: 1,
  presence: 2,
  joining: 3,
  loading: 4,
};

/** Who is playing right now, from Nocta's own clients and from Roblox's online status. */
export function playing(snapshot: AppSnapshot, accounts: Account[]) {
  const byId = new Map(accounts.map((account) => [account.id, account]));
  const rows: PlayingRow[] = snapshot.instances
    .filter(
      (instance) =>
        instance.status === "running" || instance.status === "starting",
    )
    .map((instance) => ({
      key: instance.id,
      accountId: instance.accountId,
      account: byId.get(instance.accountId) ?? null,
      username: instance.username,
      placeId: instance.placeId || null,
      location: null,
      stage: instance.status === "starting" ? "loading" : instance.stage,
      since: instance.startedAt,
      instance,
    }));
  const covered = new Set(rows.map((row) => row.accountId));
  for (const account of accounts) {
    if (covered.has(account.id) || account.presence?.userPresenceType !== 2)
      continue;
    rows.push({
      key: `presence-${account.id}`,
      accountId: account.id,
      account,
      username: account.username,
      placeId: account.presence.placeId,
      location: account.presence.lastLocation,
      stage: "presence",
      since: null,
      instance: null,
    });
  }
  return rows.sort(
    (a, b) =>
      STAGE_RANK[a.stage] - STAGE_RANK[b.stage] ||
      a.username.localeCompare(b.username),
  );
}

/** Everything the overview shows, derived once from the snapshot. */
export function overview(snapshot: AppSnapshot, now: number) {
  const accounts = snapshot.accounts.filter((account) => !account.deletedAt);
  const counts = healthCounts(accounts);
  const attention = NEEDS_ATTENTION.reduce(
    (sum, health) => sum + counts[health],
    0,
  );
  const checkedToday = accounts.filter(
    (account) =>
      account.checkedAt && now - Date.parse(account.checkedAt) < DAY_MS,
  ).length;
  const activeTasks = snapshot.tasks.filter(taskActive);
  const ownedClients = snapshot.instances.filter(
    (instance) => instance.status !== "exited",
  ).length;

  const folders = snapshot.folders
    .map((folder) => {
      const members = accounts.filter(
        (account) => account.folderId === folder.id,
      );
      return { folder, total: members.length, counts: healthCounts(members) };
    })
    .filter((item) => item.total > 0)
    .sort((a, b) => b.total - a.total);
  const unfiled = accounts.filter((account) => account.folderId === null);
  const players = playing(snapshot, accounts);
  const withRobux = accounts.filter((account) => account.robux !== null);

  return {
    accounts,
    trash: snapshot.accounts.length - accounts.length,
    counts,
    attention,
    checkedToday,
    neverChecked: accounts.filter((account) => !account.checkedAt).length,
    activeTasks,
    ownedClients,
    externalClients: snapshot.externalInstances.length,
    folders,
    unfiled: { total: unfiled.length, counts: healthCounts(unfiled) },
    players,
    inGame: players.filter(
      (row) => row.stage === "in_game" || row.stage === "presence",
    ).length,
    disconnected: players.filter((row) => row.stage === "disconnected").length,
    gettingIn: players.filter(
      (row) => row.stage === "loading" || row.stage === "joining",
    ).length,
    // Roblox's online status, plus accounts whose client Nocta runs (presence may be older).
    online: accounts.filter(
      (account) =>
        (account.presence?.userPresenceType ?? 0) > 0 ||
        players.some((row) => row.accountId === account.id),
    ).length,
    robux: withRobux.reduce((sum, account) => sum + (account.robux ?? 0), 0),
    clientMemory: snapshot.instances
      .filter((instance) => instance.status !== "exited")
      .reduce((sum, instance) => sum + (instance.memoryMb ?? 0), 0),
    robuxKnown: withRobux.length,
  };
}

export type Overview = ReturnType<typeof overview>;
