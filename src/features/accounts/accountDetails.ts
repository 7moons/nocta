import type { Account } from "@/generated/contracts";

export const ACCOUNT_COLORS = [
  "#ef4444",
  "#f97316",
  "#eab308",
  "#22c55e",
  "#14b8a6",
  "#3b82f6",
  "#8b5cf6",
  "#ec4899",
];

/** Sessions not checked for this long may have expired without Nocta noticing. */
export const STALE_AFTER_DAYS = 14;
const DAY_MS = 24 * 60 * 60 * 1000;

/** Whether a session that was checked once has not been checked or used for
 * `STALE_AFTER_DAYS`. Never-checked accounts already show "Unchecked". */
export function sessionStale(account: Account, now = Date.now()) {
  if (
    !account.hasCookie ||
    account.health === "unchecked" ||
    account.health === "invalid_session"
  )
    return false;
  const last = Math.max(
    Date.parse(account.checkedAt ?? "") || 0,
    Date.parse(account.lastStartedAt ?? "") || 0,
  );
  return now - last > STALE_AFTER_DAYS * DAY_MS;
}

/** The user's own order (dragged in the list): positioned accounts first, the rest by ID. */
export function ownOrder<T extends Pick<Account, "id" | "sortOrder">>(
  accounts: T[],
) {
  if (!accounts.some((account) => account.sortOrder !== null)) return accounts;
  return [...accounts].sort(
    (a, b) =>
      (a.sortOrder ?? Number.MAX_SAFE_INTEGER) -
        (b.sortOrder ?? Number.MAX_SAFE_INTEGER) || a.id - b.id,
  );
}

/**
 * The whole own order after dragging `moved` onto `target`: it goes before the target when
 * moved up and after it when moved down, as the row lands where it was dropped.
 */
export function moveInOrder(
  accounts: Pick<Account, "id" | "sortOrder">[],
  moved: number,
  target: number,
) {
  const ids = ownOrder(accounts).map((account) => account.id);
  const from = ids.indexOf(moved);
  const to = ids.indexOf(target);
  if (from === -1 || to === -1 || from === to) return null;
  ids.splice(from, 1);
  ids.splice(ids.indexOf(target) + (from < to ? 1 : 0), 0, moved);
  return ids;
}

/** Pinned accounts first, keeping the given order otherwise. */
export function pinnedFirst(accounts: Account[]) {
  if (!accounts.some((account) => account.pinned)) return accounts;
  return [
    ...accounts.filter((account) => account.pinned),
    ...accounts.filter((account) => !account.pinned),
  ];
}
