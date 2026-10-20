import { useState } from "react";
import type {
  ServerRegion,
  AppConfig,
  JoinStrategy,
  PlaceInfo,
  ServerRow,
} from "@/generated/contracts";
import { useT } from "@/i18n";
import { api } from "@/lib/ipc";
import { rememberPlaceInfo } from "@/lib/placeInfo";
import { errorMessage } from "@/lib/utils";
import { useUiPreference } from "@/hooks/useUiPreference";
import { placeFromUrl, serverUrl, type PrivateServer } from "./target";

const RECENT_LIMIT = 6;

/** Turns off a join strategy or followed player, leaving the patch empty when neither is set. */
const clearJoin = (config: AppConfig): Partial<AppConfig> => ({
  ...(config.joinStrategy ? { joinStrategy: null } : {}),
  ...(config.followUsername ? { followUsername: "" } : {}),
});
export const SAVED_GAMES_LIMIT = 20;

/**
 * Changes to the launch target made in the launch panel. Every change is saved right away;
 * `busy`, `error` and `warning` describe the last one.
 */
export function useTargetEditor(
  config: AppConfig,
  onPrivateServer: (server: PrivateServer | null) => void,
  onConfig: (patch: Partial<AppConfig>) => void,
) {
  const t = useT();
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [warning, setWarning] = useState("");
  const [recent, setRecent] = useUiPreference<string[]>("recentGames", []);

  /** Runs one change; resolves to whether it succeeded. */
  async function run(action: () => Promise<void>) {
    setBusy(true);
    setError("");
    setWarning("");
    try {
      await action();
      return true;
    } catch (failure) {
      setError(errorMessage(failure));
      return false;
    } finally {
      setBusy(false);
    }
  }
  const remember = (placeId: string) =>
    setRecent(
      [placeId, ...recent.filter((item) => item !== placeId)].slice(
        0,
        RECENT_LIMIT,
      ),
    );

  return {
    busy,
    error,
    warning,
    recent,
    setError,

    /** Picked servers must run in this part of the world; null is anywhere. */
    region(serverRegion: ServerRegion | null) {
      return run(() => api.patchConfig({ serverRegion }));
    },

    /** Switches to a game; any server, since a server belongs to one game. */
    chooseGame(placeId: string, info?: PlaceInfo) {
      if (info) rememberPlaceInfo(info);
      return run(async () => {
        await api.patchConfig({
          placeId,
          inviteUrl: "",
          ...(config.followUsername ? { followUsername: "" } : {}),
        });
        onPrivateServer(null);
        remember(placeId);
      });
    },

    /**
     * Uses a pasted link. No account is needed: share links are expanded once by the backend
     * with any signed-in account and remembered; the result names the game.
     */
    applyLink(url: string) {
      return run(async () => {
        const direct = await api.expandInviteLink(url);
        const place =
          placeFromUrl(direct) ??
          (await api
            .resolveLaunchTarget(null, { kind: "invite", url: direct })
            .then((resolved) => resolved.placeId)
            .catch(() => null));
        await api.patchConfig({
          placeId: place ?? config.placeId,
          inviteUrl: direct,
          ...clearJoin(config),
        });
        onPrivateServer(null);
        if (place) remember(place);
        else setWarning(t("launch.game.linkSavedWithoutGame"));
      });
    },

    anyServer() {
      onPrivateServer(null);
      if (config.inviteUrl || config.joinStrategy || config.followUsername)
        void run(() =>
          api.patchConfig({ inviteUrl: "", ...clearJoin(config) }),
        );
    },

    /** Lets Nocta choose the server when launching: squad, lowest ping or a hop. */
    strategy(strategy: JoinStrategy) {
      onPrivateServer(null);
      void run(() =>
        api.patchConfig({
          joinStrategy: strategy,
          inviteUrl: "",
          followUsername: "",
        }),
      );
    },

    /** Joins whatever server this player is in; checks the username exists first. */
    follow(username: string) {
      return run(async () => {
        const user = await api.findUser(username);
        onPrivateServer(null);
        await api.patchConfig({
          followUsername: user.name,
          joinStrategy: null,
          inviteUrl: "",
        });
      });
    },

    pickServer(server: ServerRow) {
      if (server.kind === "vip") {
        if (!server.targetId) return;
        onPrivateServer({
          targetId: server.targetId,
          placeId: config.placeId,
          name: server.name ?? t("launch.server.privateFallbackName"),
          at: Date.now(),
        });
        if (config.inviteUrl || config.joinStrategy || config.followUsername)
          void run(() =>
            api.patchConfig({ inviteUrl: "", ...clearJoin(config) }),
          );
        return;
      }
      onPrivateServer(null);
      void run(() =>
        api.patchConfig({
          inviteUrl: serverUrl(config.placeId, server.id),
          ...clearJoin(config),
        }),
      );
    },

    toggleSaved(name: string) {
      const saved = config.presets.find(
        (item) => item.placeId === config.placeId,
      );
      if (saved)
        onConfig({
          presets: config.presets.filter((item) => item.id !== saved.id),
        });
      else if (config.presets.length < SAVED_GAMES_LIMIT)
        onConfig({
          presets: [
            ...config.presets,
            { id: crypto.randomUUID(), placeId: config.placeId, name },
          ],
        });
    },
  };
}
