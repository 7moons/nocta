import type {
  AppConfig,
  JoinStrategy,
  LaunchTargetInput,
} from "@/generated/contracts";
import type { Translate } from "@/i18n";

/** A private server picked from the list. Its access code stays in the backend behind an
 * opaque ID that expires, so it lives only for this session instead of in the config. */
export interface PrivateServer {
  targetId: string;
  placeId: string;
  name: string;
  at: number;
}

// The backend forgets opaque server IDs after 30 minutes; stop using them a little earlier.
const PRIVATE_TTL_MS = 25 * 60 * 1000;

export const privateServerUsable = (server: PrivateServer | null) =>
  !!server && Date.now() - server.at < PRIVATE_TTL_MS;

/** The private server only applies while the game it belongs to is still selected. */
export function privateServerFor(
  server: PrivateServer | null,
  placeId: string,
): PrivateServer | null {
  return server && privateServerUsable(server) && server.placeId === placeId
    ? server
    : null;
}

/** Where a launch goes: the picked private server, the player to follow, a server Nocta
 * chooses by the join strategy, or the saved game and link. */
export function launchTarget(
  config: AppConfig,
  server: PrivateServer | null,
): LaunchTargetInput {
  if (server && privateServerUsable(server))
    return { kind: "server", targetId: server.targetId };
  if (config.followUsername)
    return { kind: "follow", username: config.followUsername };
  if (config.joinStrategy && config.placeId)
    return {
      kind: "pick",
      placeId: config.placeId,
      strategy: config.joinStrategy,
      excludeJob: null,
    };
  return { kind: "configured" };
}

/** Whether there is anywhere to launch to. */
export const hasTarget = (config: AppConfig) =>
  !!config.placeId || !!config.inviteUrl || !!config.followUsername;

export const STRATEGY_KEYS = {
  squad: "squad",
  lowest_ping: "lowestPing",
  hop: "hop",
  fullest: "fullest",
  emptiest: "emptiest",
} as const satisfies Record<JoinStrategy, string>;

export function jobFromUrl(value: string) {
  try {
    const url = new URL(value);
    return (
      url.searchParams.get("gameInstanceId") ?? url.searchParams.get("jobId")
    );
  } catch {
    return null;
  }
}

export const serverUrl = (placeId: string, jobId: string) =>
  `https://www.roblox.com/games/${placeId}/?gameInstanceId=${encodeURIComponent(jobId)}`;

export const placeFromUrl = (url: string) =>
  url.match(/roblox\.com\/games\/(\d+)/i)?.[1] ?? null;

/** How accounts join the selected game: any server, a picked one, a pasted link, a server
 * Nocta chooses when launching, or the server a player is in. */
export type JoinMode = "any" | "pick" | "link" | JoinStrategy | "follow";

export function joinMode(
  config: AppConfig,
  privateServer: PrivateServer | null,
): JoinMode {
  if (privateServerFor(privateServer, config.placeId)) return "pick";
  if (config.followUsername) return "follow";
  if (config.joinStrategy) return config.joinStrategy;
  if (jobFromUrl(config.inviteUrl)) return "pick";
  return config.inviteUrl ? "link" : "any";
}

/** How the account will join, in words, for the toolbar and the panel. */
export function describeServer(
  t: Translate,
  config: AppConfig,
  server: PrivateServer | null,
) {
  const privateServer = privateServerFor(server, config.placeId);
  if (privateServer)
    return t("launch.server.private", { name: privateServer.name });
  if (config.followUsername)
    return t("launch.join.followSummary", { name: config.followUsername });
  if (config.joinStrategy)
    return t(`launch.join.${STRATEGY_KEYS[config.joinStrategy]}`);
  const job = jobFromUrl(config.inviteUrl);
  if (job) return t("launch.server.job", { id: job.slice(0, 8) });
  if (/[?&]privateServerLinkCode=/i.test(config.inviteUrl))
    return t("launch.server.privateLink");
  if (config.inviteUrl) return t("launch.server.invite");
  return t("launch.server.any");
}

export type GameQuery =
  | { kind: "empty" }
  | { kind: "place"; placeId: string }
  | { kind: "link"; url: string }
  | { kind: "invalid" }
  | { kind: "search"; text: string };

const ROBLOX_HOSTS = ["roblox.com", "www.roblox.com", "web.roblox.com"];
const SERVER_PARAMS = ["gameInstanceId", "jobId", "privateServerLinkCode"];

/** What was typed: a Place ID, a Roblox link (game, server or private server), or a name. */
export function classifyGameQuery(raw: string): GameQuery {
  const text = raw.trim();
  if (!text) return { kind: "empty" };
  if (/^[1-9]\d{0,18}$/.test(text)) return { kind: "place", placeId: text };
  const hasScheme = /^[a-z]+:\/\//i.test(text);
  if (!hasScheme && !/^(www\.|web\.)?roblox\.com\//i.test(text))
    return { kind: "search", text };
  try {
    const url = new URL(hasScheme ? text : `https://${text}`);
    const roblox = ROBLOX_HOSTS.includes(url.hostname);
    if (
      url.protocol !== "https:" ||
      !(roblox || url.hostname === "ro.blox.com") ||
      url.username ||
      url.password ||
      url.port
    )
      return { kind: "invalid" };
    // A plain game page is just the game; anything that names a server is a link that the
    // backend resolves, including share links and private servers.
    const game = url.pathname.match(/^\/games\/(\d+)/i)?.[1];
    if (
      roblox &&
      game &&
      !SERVER_PARAMS.some((name) => url.searchParams.has(name))
    )
      return { kind: "place", placeId: game };
    return { kind: "link", url: url.toString() };
  } catch {
    return { kind: "invalid" };
  }
}
