import { ChevronDown, Play } from "lucide-react";
import type { AppConfig } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { GameThumb } from "./GameIcon";
import { describeServer } from "./target";
import type { useLauncher } from "./useLauncher";

/** Toolbar pair: the current target (opens the launch panel) and "Launch selected". */
export function LaunchControl({
  config,
  launcher,
  selected,
  canLaunch,
}: {
  config: AppConfig;
  launcher: ReturnType<typeof useLauncher>;
  selected: number[];
  canLaunch: boolean;
}) {
  const t = useT();
  const hasTarget = !!config.placeId || !!config.inviteUrl;
  return (
    <div className="launch-control">
      <button
        className={`launch-target-button ${launcher.open ? "active" : ""}`}
        aria-label={t("launch.target.change", { game: launcher.targetName })}
        aria-expanded={launcher.open}
        title={t("launch.target.tooltip")}
        onClick={launcher.toggle}
      >
        <GameThumb src={launcher.game?.iconUrl} size={14} />
        <span>
          <strong>{launcher.targetName}</strong>
          <small>
            {hasTarget
              ? describeServer(t, config, launcher.privateServer)
              : t("launch.target.noGame")}
          </small>
        </span>
        <ChevronDown size={14} />
      </button>
      <Button
        disabled={!selected.length || !canLaunch}
        title={selected.length ? undefined : t("launch.selectFirst")}
        onClick={() => launcher.launch(selected)}
      >
        <Play />
        {t("launch.launchSelected")}
        {selected.length > 0 && (
          <span className="button-count">{selected.length}</span>
        )}
      </Button>
    </div>
  );
}
