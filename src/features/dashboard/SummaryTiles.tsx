import {
  AlertTriangle,
  Coins,
  Gamepad2,
  MonitorPlay,
  type LucideIcon,
} from "lucide-react";
import { useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import type { Overview } from "./overview";

/** The four numbers that matter most, each leading to where they can be acted on. */
export function SummaryTiles({
  data,
  onAccounts,
  onPlaying,
  onActivity,
}: {
  data: Overview;
  onAccounts: (health?: string) => void;
  onPlaying: () => void;
  onActivity: () => void;
}) {
  const t = useT();
  const format = useFormat();
  const total = data.accounts.length;
  const clients = data.ownedClients + data.externalClients;
  return (
    <div className="overview-tiles">
      <Tile
        icon={Gamepad2}
        tone={data.inGame > 0 ? "good" : undefined}
        label={t("dashboard.tiles.inGame")}
        value={format.number(data.inGame)}
        detail={t("dashboard.tiles.inGameDetail", {
          online: data.online,
          total,
        })}
        onClick={onPlaying}
      />
      <Tile
        icon={MonitorPlay}
        tone={
          data.disconnected > 0
            ? "warn"
            : clients > 0 || data.activeTasks.length > 0
              ? "active"
              : undefined
        }
        label={t("dashboard.tiles.clients")}
        value={format.number(clients)}
        detail={
          data.disconnected > 0
            ? t("dashboard.tiles.clientsDisconnected", {
                count: data.disconnected,
              })
            : data.clientMemory > 0
              ? t("dashboard.tiles.clientsMemory", {
                  memory: format.memory(data.clientMemory),
                  external: data.externalClients,
                })
              : t("dashboard.tiles.clientsDetail", {
                  tasks: data.activeTasks.length,
                  external: data.externalClients,
                })
        }
        onClick={onActivity}
      />
      <Tile
        icon={Coins}
        label={t("dashboard.tiles.robux")}
        value={data.robuxKnown ? format.number(data.robux) : "—"}
        detail={
          data.robuxKnown
            ? t("dashboard.tiles.robuxDetail", { count: data.robuxKnown })
            : t("dashboard.tiles.robuxUnknown")
        }
        onClick={() => onAccounts()}
      />
      <Tile
        icon={AlertTriangle}
        tone={data.attention > 0 ? "warn" : undefined}
        label={t("dashboard.tiles.attention")}
        value={format.number(data.attention)}
        detail={
          data.attention > 0
            ? t("dashboard.tiles.attentionDetail", {
                captcha: data.counts.captcha_required,
                invalid: data.counts.invalid_session + data.counts.error,
              })
            : t("dashboard.tiles.attentionNone")
        }
        onClick={() =>
          onAccounts(
            data.counts.captcha_required > 0
              ? "captcha_required"
              : data.counts.invalid_session > 0
                ? "invalid_session"
                : "error",
          )
        }
        disabled={data.attention === 0}
      />
    </div>
  );
}

function Tile({
  icon: Icon,
  label,
  value,
  detail,
  tone,
  disabled,
  onClick,
}: {
  icon: LucideIcon;
  label: string;
  value: string;
  detail: string;
  tone?: "good" | "warn" | "active";
  disabled?: boolean;
  onClick: () => void;
}) {
  return (
    <button
      className={`overview-tile ${tone ? `tone-${tone}` : ""}`}
      disabled={disabled}
      onClick={onClick}
    >
      <span className="overview-tile-label">
        <Icon size={14} />
        {label}
      </span>
      <strong>{value}</strong>
      <small>{detail}</small>
    </button>
  );
}
