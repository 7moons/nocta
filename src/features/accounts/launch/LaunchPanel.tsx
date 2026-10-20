import { useEffect } from "react";
import { X } from "lucide-react";
import type { AppConfig, LauncherState } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { usePlaceInfo } from "@/lib/placeInfo";
import { GamePicker } from "./GamePicker";
import { LaunchFooter } from "./LaunchFooter";
import { ServerPicker } from "./ServerPicker";
import { describeServer, hasTarget, type PrivateServer } from "./target";
import { useTargetEditor } from "./useTargetEditor";

/**
 * One place to decide where accounts launch: pick a game, then how to join it. Everything
 * saves as it changes; the footer launches the selected accounts right here.
 */
export function LaunchPanel({
  config,
  launcher,
  accountId,
  accountName,
  selectedCount,
  privateServer,
  notice,
  onPrivateServer,
  onConfig,
  onLaunch,
  onClose,
}: {
  config: AppConfig;
  launcher: LauncherState;
  /** First selected account: lists its private servers and resolves links needing a session. */
  accountId: number | null;
  accountName: string | null;
  selectedCount: number;
  privateServer: PrivateServer | null;
  /** Why the panel opened by itself, if it did. */
  notice: string;
  onPrivateServer: (server: PrivateServer | null) => void;
  onConfig: (patch: Partial<AppConfig>) => void;
  onLaunch: () => void;
  onClose: () => void;
}) {
  const t = useT();
  const game = usePlaceInfo(config.placeId);
  const editor = useTargetEditor(config, onPrivateServer, onConfig);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !event.defaultPrevented) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const summary = config.followUsername
    ? describeServer(t, config, privateServer)
    : config.placeId
      ? `${game?.name ?? t("launch.game.placeFallback", { id: config.placeId })} · ${describeServer(t, config, privateServer)}`
      : t("launch.chooseGameFirst");

  return (
    <>
      <div className="launch-sheet-backdrop" onClick={onClose} />
      <aside className="launch-sheet" aria-label={t("launch.panel")}>
        <header className="launch-sheet-header">
          <div>
            <h2>{t("launch.title")}</h2>
            <p>{t("launch.subtitle")}</p>
          </div>
          <Button
            variant="ghost"
            size="icon"
            aria-label={t("launch.close")}
            onClick={onClose}
          >
            <X />
          </Button>
        </header>
        <div className="launch-sheet-scroll">
          {notice && (
            <p className="launch-notice" role="status">
              {notice}
            </p>
          )}
          <GamePicker config={config} game={game} editor={editor} />
          {(config.placeId || config.followUsername) && (
            <ServerPicker
              config={config}
              privateServer={privateServer}
              accountId={accountId}
              accountName={accountName}
              editor={editor}
            />
          )}
        </div>
        <LaunchFooter
          config={config}
          launcher={launcher}
          selectedCount={selectedCount}
          ready={hasTarget(config)}
          summary={summary}
          onConfig={onConfig}
          onLaunch={onLaunch}
        />
      </aside>
    </>
  );
}
