import { useEffect, useMemo, useState } from "react";
import {
  LoaderCircle,
  LogIn,
  RefreshCw,
  Search,
  UserRound,
} from "lucide-react";
import type { AppSnapshot, FriendEntry } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { ViewHeader } from "@/components/ViewHeader";
import { useUiPreference } from "@/hooks/useUiPreference";
import { useT } from "@/i18n";
import type { RunAction } from "@/lib/actions";
import { api } from "@/lib/ipc";
import { usePlaceInfo } from "@/lib/placeInfo";
import { errorMessage } from "@/lib/utils";
import { usePrivacy } from "@/lib/privacy";

const PRESENCE = ["offline", "online", "inGame", "inStudio"] as const;

/** The friends of an account, where they are, and joining them. */
export function FriendsView({
  snapshot,
  run,
}: {
  snapshot: AppSnapshot;
  run: RunAction;
}) {
  const t = useT();
  const privacy = usePrivacy();
  // Friends can only be listed for accounts whose Roblox ID and session are known.
  const accounts = useMemo(
    () =>
      snapshot.accounts.filter(
        (account) => !account.deletedAt && account.userId && account.hasCookie,
      ),
    [snapshot.accounts],
  );
  const [saved, setSaved] = useUiPreference<number | null>(
    "friendsAccount",
    null,
  );
  const accountId =
    accounts.find((account) => account.id === saved)?.id ??
    accounts[0]?.id ??
    null;
  const [state, setState] = useState<{
    for: number | null;
    friends: FriendEntry[] | null;
    error: string;
  }>({ for: null, friends: null, error: "" });
  const [reload, setReload] = useState(0);
  const [search, setSearch] = useState("");
  const [onlyInGame, setOnlyInGame] = useUiPreference("friendsInGame", false);
  const loading = accountId !== null && state.for !== accountId;

  useEffect(() => {
    if (accountId === null) return;
    let live = true;
    api
      .friendsOf(accountId)
      .then(
        (friends) => live && setState({ for: accountId, friends, error: "" }),
      )
      .catch(
        (failure) =>
          live &&
          setState({
            for: accountId,
            friends: null,
            error: errorMessage(failure),
          }),
      );
    return () => {
      live = false;
    };
  }, [accountId, reload]);

  const shown = (state.friends ?? []).filter(
    (friend) =>
      (!onlyInGame || friend.presence?.kind === 2) &&
      `${friend.user.name} ${friend.user.displayName}`
        .toLowerCase()
        .includes(search.trim().toLowerCase()),
  );
  const inGame = (state.friends ?? []).filter(
    (friend) => friend.presence?.kind === 2,
  ).length;

  return (
    <>
      <ViewHeader
        icon={UserRound}
        title={t("views.friends")}
        subtitle={
          state.friends
            ? t("friends.summary", {
                count: state.friends.length,
                inGame,
              })
            : t("friends.subtitle")
        }
        actions={
          <>
            <select
              aria-label={t("friends.account")}
              value={accountId ?? ""}
              disabled={!accounts.length}
              onChange={(event) => setSaved(Number(event.target.value))}
            >
              {accounts.map((account) => (
                <option key={account.id} value={account.id}>
                  {privacy.name(account)}
                </option>
              ))}
            </select>
            <Button
              variant="outline"
              disabled={accountId === null || loading}
              onClick={() => {
                setState((value) => ({ ...value, for: null }));
                setReload(reload + 1);
              }}
            >
              <RefreshCw className={loading ? "spin" : undefined} />
              {t("friends.refresh")}
            </Button>
          </>
        }
      />
      <div className="standard-content friends-view">
        <div className="table-tools">
          <label className="search-field">
            <Search size={13} />
            <input
              aria-label={t("friends.search")}
              placeholder={t("friends.search")}
              value={search}
              onChange={(event) => setSearch(event.target.value)}
            />
          </label>
          <label className="checkbox-label">
            <input
              type="checkbox"
              checked={onlyInGame}
              onChange={(event) => setOnlyInGame(event.target.checked)}
            />
            {t("friends.onlyInGame")}
          </label>
        </div>
        {accountId === null && (
          <p className="empty-state">{t("friends.noAccounts")}</p>
        )}
        {loading && (
          <p className="empty-state">
            <LoaderCircle className="spin" size={14} /> {t("friends.loading")}
          </p>
        )}
        {!loading && state.error && (
          <p className="error-banner" role="alert">
            {state.error}
          </p>
        )}
        {!loading && state.friends && shown.length === 0 && (
          <p className="empty-state">{t("friends.empty")}</p>
        )}
        {!loading && shown.length > 0 && (
          <ul className="friend-list" aria-label={t("views.friends")}>
            {shown.map((friend) => (
              <FriendRow
                key={friend.user.id}
                friend={friend}
                onJoin={() =>
                  accountId !== null &&
                  run(
                    () =>
                      api.launchGame([accountId], {
                        kind: "follow",
                        username: friend.user.name,
                      }),
                    t("friends.joining", { name: friend.user.displayName }),
                  )
                }
              />
            ))}
          </ul>
        )}
      </div>
    </>
  );
}

function FriendRow({
  friend,
  onJoin,
}: {
  friend: FriendEntry;
  onJoin: () => void;
}) {
  const t = useT();
  const presence = friend.presence;
  const state = PRESENCE[presence?.kind ?? 0] ?? "offline";
  const game = usePlaceInfo(
    state === "inGame" ? (presence?.placeId ?? "") : "",
  );
  const where =
    state === "inGame"
      ? (presence?.location ?? game?.name ?? t("friends.hiddenGame"))
      : null;
  return (
    <li className={`friend-row ${state}`}>
      <span className="avatar friend-avatar">
        {friend.avatarUrl ? (
          <img src={friend.avatarUrl} alt="" loading="lazy" />
        ) : (
          friend.user.name.slice(0, 1).toUpperCase()
        )}
        {state !== "offline" && (
          <span
            className={`presence-dot ${state === "inGame" ? "in-game" : state === "inStudio" ? "in-studio" : "online"}`}
          />
        )}
      </span>
      <div className="friend-name">
        <strong>{friend.user.displayName}</strong>
        <small>@{friend.user.name}</small>
      </div>
      <span className={`friend-status ${state}`}>
        {t(`accounts.presence.${state}`)}
        {where && <span title={where}> · {where}</span>}
      </span>
      <Button
        size="sm"
        variant={state === "inGame" ? "default" : "outline"}
        disabled={state !== "inGame"}
        aria-label={t("friends.joinName", { name: friend.user.displayName })}
        title={state === "inGame" ? undefined : t("friends.notInGame")}
        onClick={onJoin}
      >
        <LogIn />
        {t("friends.join")}
      </Button>
    </li>
  );
}
