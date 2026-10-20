import { Gamepad2, LayoutGrid, Play, X } from "lucide-react";
import type { Tone } from "@/app/activity/ActivityLine";
import { Button } from "@/components/ui/button";
import { AccountAvatar } from "@/features/accounts/AccountName";
import { useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import { usePlaceInfo } from "@/lib/placeInfo";
import { usePrivacy } from "@/lib/privacy";
import { OverviewCard } from "./parts";
import type { Overview, PlayingRow } from "./overview";

const TONES: Record<PlayingRow["stage"], Tone> = {
  in_game: "success",
  presence: "success",
  joining: "warning",
  loading: "warning",
  disconnected: "danger",
};

/** How long ago `since` was, as "2 h 5 min" or "12 min". */
function elapsed(since: string, now: number) {
  const minutes = Math.max(0, Math.floor((now - Date.parse(since)) / 60_000));
  return { hours: Math.floor(minutes / 60), minutes: minutes % 60 };
}

/** The accounts in a game right now, with where they are and for how long. */
export function PlayingCard({
  data,
  now,
  onLaunch,
  onArrange,
  onClose,
}: {
  data: Overview;
  now: number;
  onLaunch: () => void;
  onArrange: () => void;
  onClose: (instanceId: string) => void;
}) {
  const t = useT();
  const owned = data.players.some((row) => row.instance);
  return (
    <OverviewCard
      className="overview-playing"
      title={t("dashboard.playing.title")}
      subtitle={
        data.players.length
          ? t("dashboard.playing.subtitle", {
              inGame: data.inGame,
              gettingIn: data.gettingIn,
              disconnected: data.disconnected,
            })
          : undefined
      }
      action={
        owned && (
          <Button variant="outline" size="sm" onClick={onArrange}>
            <LayoutGrid />
            {t("activity.arrange")}
          </Button>
        )
      }
    >
      {data.players.length === 0 ? (
        <div className="overview-empty">
          <Gamepad2 size={22} />
          <p>{t("dashboard.playing.empty")}</p>
          <Button variant="outline" size="sm" onClick={onLaunch}>
            <Play />
            {t("dashboard.playing.launch")}
          </Button>
        </div>
      ) : (
        <ul className="playing-list" aria-label={t("dashboard.playing.title")}>
          {data.players.map((row) => (
            <PlayingLine key={row.key} row={row} now={now} onClose={onClose} />
          ))}
        </ul>
      )}
    </OverviewCard>
  );
}

function PlayingLine({
  row,
  now,
  onClose,
}: {
  row: PlayingRow;
  now: number;
  onClose: (instanceId: string) => void;
}) {
  const t = useT();
  const format = useFormat();
  const privacy = usePrivacy();
  const info = usePlaceInfo(row.placeId);
  const name = row.account
    ? privacy.name(row.account)
    : privacy.nameOf(row.accountId, row.username);
  const game =
    info?.name ??
    row.location ??
    (row.placeId
      ? t("launch.game.placeFallback", { id: row.placeId })
      : t("dashboard.playing.unknownGame"));
  const time = row.since ? elapsed(row.since, now) : null;
  return (
    <li className="playing-line">
      {row.account ? (
        <AccountAvatar account={row.account} />
      ) : (
        <span className="avatar">{name.slice(0, 1).toUpperCase()}</span>
      )}
      <span className="playing-who">
        <strong title={name}>{name}</strong>
        <small title={game}>
          {info?.iconUrl && <img src={info.iconUrl} alt="" />}
          {game}
        </small>
      </span>
      <span className={`activity-pill tone-${TONES[row.stage]}`}>
        {row.stage === "presence"
          ? t("dashboard.playing.elsewhere")
          : t(`activity.stage.${row.stage}`)}
      </span>
      <small className="playing-time">
        {time ? t("accounts.playtime.value", time) : ""}
        {row.instance?.memoryMb != null && (
          <span>{format.memory(row.instance.memoryMb)}</span>
        )}
      </small>
      {row.instance ? (
        <Button
          variant="ghost"
          size="icon"
          aria-label={t("activity.closeInstance", { name })}
          title={t("activity.closeInstance", { name })}
          onClick={() => onClose(row.instance!.id)}
        >
          <X />
        </Button>
      ) : (
        <span />
      )}
    </li>
  );
}
