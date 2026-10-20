import { useState } from "react";
import type { AppConfig } from "@/generated/contracts";
import { useT } from "@/i18n";
import type { RunAction } from "@/lib/actions";
import { api } from "@/lib/ipc";
import { usePlaceInfo } from "@/lib/placeInfo";
import {
  hasTarget,
  launchTarget,
  privateServerFor,
  privateServerUsable,
  type PrivateServer,
} from "./target";

/**
 * Launching from the accounts view: the panel's open state, the session-only private server
 * pick and `launch`, which goes straight to the chosen target. Without a usable target it opens
 * the panel and says why instead.
 */
export function useLauncher(config: AppConfig, run: RunAction) {
  const t = useT();
  const [open, setOpen] = useState(false);
  const [notice, setNotice] = useState("");
  const [privateServer, setPrivateServer] = useState<PrivateServer | null>(
    null,
  );
  const game = usePlaceInfo(config.placeId);
  // The game name with its icon; the raw ID only shows while the name is unknown.
  const targetName = config.followUsername
    ? t("launch.join.followSummary", { name: config.followUsername })
    : config.placeId
      ? (game?.name ?? t("launch.game.placeFallback", { id: config.placeId }))
      : t(config.inviteUrl ? "launch.target.inviteOnly" : "launch.target.none");

  function openWith(message: string) {
    setNotice(message);
    setOpen(true);
  }

  function launch(accountIds: number[]) {
    if (!hasTarget(config)) return openWith(t("launch.chooseGameThenLaunch"));
    const forThisGame = privateServer?.placeId === config.placeId;
    if (forThisGame && !privateServerUsable(privateServer)) {
      setPrivateServer(null);
      return openWith(t("launch.privateExpired"));
    }
    run(
      () =>
        api.launchGame(
          accountIds,
          launchTarget(config, privateServerFor(privateServer, config.placeId)),
        ),
      t("launch.launching", {
        count: accountIds.length,
        game: config.followUsername
          ? t("launch.join.followTarget", { name: config.followUsername })
          : targetName,
      }),
    );
  }

  return {
    open,
    notice,
    privateServer,
    game,
    targetName,
    setPrivateServer,
    toggle() {
      setNotice("");
      setOpen(!open);
    },
    close: () => setOpen(false),
    launch,
  };
}
