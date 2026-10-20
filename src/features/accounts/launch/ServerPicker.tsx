import { useMemo, useState } from "react";
import {
  Check,
  ChevronsDown,
  ChevronsUp,
  Link2,
  LoaderCircle,
  Lock,
  Gauge,
  RefreshCw,
  Repeat,
  Shuffle,
  UserRoundSearch,
  Users,
  UsersRound,
  type LucideIcon,
} from "lucide-react";
import type {
  ServerRegion,
  AppConfig,
  JoinStrategy,
  ServerRow,
} from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useUiPreference } from "@/hooks/useUiPreference";
import { useT } from "@/i18n";
import {
  jobFromUrl,
  joinMode,
  privateServerFor,
  STRATEGY_KEYS,
  type JoinMode,
  type PrivateServer,
} from "./target";
import { useServerList } from "./useServerList";
import type { useTargetEditor } from "./useTargetEditor";

/** Step 2 of the launch panel: how accounts join the selected game. */
const REGIONS: ServerRegion[] = [
  "europe",
  "north_america",
  "south_america",
  "asia",
  "oceania",
  "africa",
];

export function ServerPicker({
  config,
  privateServer,
  accountId,
  accountName,
  editor,
}: {
  config: AppConfig;
  privateServer: PrivateServer | null;
  accountId: number | null;
  accountName: string | null;
  editor: ReturnType<typeof useTargetEditor>;
}) {
  const t = useT();
  const mode = joinMode(config, privateServer);
  // "Choose a server" stays open before a server is picked; a saved change resets it.
  const [view, setView] = useState<{ mode: JoinMode; for: string }>({
    mode,
    for: `${mode}:${config.placeId}`,
  });
  const shown = view.for === `${mode}:${config.placeId}` ? view.mode : mode;
  const select = (next: JoinMode) =>
    setView({ mode: next, for: `${mode}:${config.placeId}` });
  const picked = privateServerFor(privateServer, config.placeId);

  const game = !!config.placeId;
  const strategies: { value: JoinStrategy; icon: LucideIcon }[] = [
    { value: "squad", icon: UsersRound },
    { value: "lowest_ping", icon: Gauge },
    { value: "hop", icon: Repeat },
    { value: "fullest", icon: ChevronsUp },
    { value: "emptiest", icon: ChevronsDown },
  ];

  return (
    <section className="launch-step" aria-label={t("launch.server.title")}>
      <h3>
        <span className="launch-step-number">2</span>
        {t("launch.server.title")}
      </h3>
      <div
        className="join-options"
        role="radiogroup"
        aria-label={t("launch.server.howToJoin")}
      >
        {game && (
          <>
            <JoinOption
              icon={Shuffle}
              title={t("launch.server.any")}
              detail={t("launch.server.anyDetail")}
              checked={shown === "any"}
              onSelect={() => {
                select("any");
                editor.anyServer();
              }}
            />
            <JoinOption
              icon={Users}
              title={t("launch.server.choose")}
              detail={t("launch.server.chooseDetail")}
              checked={shown === "pick"}
              onSelect={() => select("pick")}
            />
            {strategies.map(({ value, icon }) => (
              <JoinOption
                key={value}
                icon={icon}
                title={t(`launch.join.${STRATEGY_KEYS[value]}`)}
                detail={t(`launch.join.${STRATEGY_KEYS[value]}Detail`)}
                checked={shown === value}
                onSelect={() => {
                  select(value);
                  editor.strategy(value);
                }}
              />
            ))}
          </>
        )}
        <JoinOption
          icon={UserRoundSearch}
          title={t("launch.join.follow")}
          detail={t("launch.join.followDetail")}
          checked={shown === "follow"}
          onSelect={() => select("follow")}
        />
        {mode === "link" && (
          <JoinOption
            icon={Link2}
            title={t("launch.server.inviteOption")}
            detail={t("launch.server.inviteDetail")}
            checked={shown === "link"}
            onSelect={() => select("link")}
          />
        )}
      </div>
      {game && strategies.some((item) => item.value === shown) && (
        <label className="field-label server-region">
          {t("launch.region.label")}
          <select
            aria-label={t("launch.region.label")}
            value={config.serverRegion ?? ""}
            disabled={editor.busy}
            onChange={(event) =>
              void editor.region(
                (event.target.value || null) as ServerRegion | null,
              )
            }
          >
            <option value="">{t("launch.region.any")}</option>
            {REGIONS.map((region) => (
              <option key={region} value={region}>
                {t(`launch.region.${region}`)}
              </option>
            ))}
          </select>
          <small className="field-hint">{t("launch.region.detail")}</small>
        </label>
      )}
      {shown === "pick" && game && (
        <ServerList
          placeId={config.placeId}
          accountId={accountId}
          accountName={accountName}
          selectedJob={picked ? null : jobFromUrl(config.inviteUrl)}
          selectedPrivate={picked?.targetId ?? null}
          disabled={editor.busy}
          onPick={editor.pickServer}
          onError={editor.setError}
        />
      )}
      {shown === "follow" && (
        <FollowPlayer
          current={config.followUsername}
          busy={editor.busy}
          onFollow={editor.follow}
        />
      )}
    </section>
  );
}

/** The username of the player to join; checked against Roblox when saved. */
function FollowPlayer({
  current,
  busy,
  onFollow,
}: {
  current: string;
  busy: boolean;
  onFollow: (username: string) => Promise<boolean>;
}) {
  const t = useT();
  const [name, setName] = useState(current);
  const changed = name.trim() !== "" && name.trim() !== current;
  return (
    <form
      className="follow-player"
      onSubmit={(event) => {
        event.preventDefault();
        if (changed) void onFollow(name.trim());
      }}
    >
      <input
        aria-label={t("launch.join.followUsername")}
        placeholder={t("launch.join.followPlaceholder")}
        value={name}
        maxLength={40}
        onChange={(event) => setName(event.target.value)}
      />
      <Button type="submit" disabled={!changed || busy}>
        {busy && <LoaderCircle className="spin" />}
        {t("launch.join.followSave")}
      </Button>
      <p className="launch-hint">
        {current
          ? t("launch.join.followCurrent", { name: current })
          : t("launch.join.followHint")}
      </p>
    </form>
  );
}

function JoinOption({
  icon: Icon,
  title,
  detail,
  checked,
  onSelect,
}: {
  icon: LucideIcon;
  title: string;
  detail: string;
  checked: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      role="radio"
      aria-checked={checked}
      className={`join-option ${checked ? "active" : ""}`}
      onClick={onSelect}
    >
      <Icon size={16} />
      <span>
        <strong>{title}</strong>
        <small>{detail}</small>
      </span>
    </button>
  );
}

// Clicking a row picks it; private servers of the first selected account stay on top.
function ServerList({
  placeId,
  accountId,
  accountName,
  selectedJob,
  selectedPrivate,
  disabled,
  onPick,
  onError,
}: {
  placeId: string;
  accountId: number | null;
  accountName: string | null;
  selectedJob: string | null;
  selectedPrivate: string | null;
  disabled: boolean;
  onPick: (server: ServerRow) => void;
  onError: (message: string) => void;
}) {
  const t = useT();
  const [sort, setSort] = useUiPreference<string>("serverSort", "most");
  const [hideFull, setHideFull] = useUiPreference("serverHideFull", true);
  const [withPrivate, setWithPrivate] = useUiPreference(
    "serverWithPrivate",
    false,
  );
  const includePrivate = withPrivate && accountId !== null;
  const list = useServerList(
    {
      placeId,
      privateFor: includePrivate ? accountId : null,
      mostPlayersFirst: sort === "most",
      hideFull,
    },
    onError,
  );
  const visible = useMemo(
    () =>
      [...(list.rows ?? [])].sort((a, b) => {
        if (a.kind !== b.kind) return a.kind === "vip" ? -1 : 1;
        return sort === "most" ? b.playing - a.playing : a.playing - b.playing;
      }),
    [list.rows, sort],
  );

  return (
    <div
      className="server-browser"
      aria-label={t("launch.servers.title")}
      role="region"
    >
      <div className="server-browser-toolbar">
        <div
          className="segmented"
          role="group"
          aria-label={t("launch.servers.sort")}
        >
          {(["most", "fewest"] as const).map((value) => (
            <button
              key={value}
              aria-pressed={sort === value}
              className={sort === value ? "active" : ""}
              onClick={() => setSort(value)}
            >
              {t(
                value === "most"
                  ? "launch.servers.most"
                  : "launch.servers.fewest",
              )}
            </button>
          ))}
        </div>
        <label className="checkbox-label">
          <input
            type="checkbox"
            checked={hideFull}
            onChange={(event) => setHideFull(event.target.checked)}
          />
          {t("launch.servers.hideFull")}
        </label>
        <label
          className="checkbox-label"
          title={
            accountId === null
              ? t("launch.servers.privateNeedsAccount")
              : undefined
          }
        >
          <input
            type="checkbox"
            checked={includePrivate}
            disabled={accountId === null}
            onChange={(event) => setWithPrivate(event.target.checked)}
          />
          {accountName
            ? t("launch.servers.privateOf", { name: accountName })
            : t("launch.servers.private")}
        </label>
        <Button
          className="ml-auto"
          variant="ghost"
          size="icon"
          aria-label={t("launch.servers.refresh")}
          title={t("launch.servers.refresh")}
          disabled={list.listing}
          onClick={list.refresh}
        >
          <RefreshCw className={list.listing ? "spin" : undefined} />
        </Button>
      </div>
      {list.warnings.map((warning) => (
        <p className="launch-hint warning" key={warning}>
          {warning}
        </p>
      ))}
      <div
        className="server-list"
        role="radiogroup"
        aria-label={t("launch.servers.title")}
      >
        {visible.map((server) => (
          <ServerOption
            key={`${server.kind}-${server.id}`}
            server={server}
            checked={
              server.kind === "vip"
                ? !!server.targetId && server.targetId === selectedPrivate
                : server.id === selectedJob
            }
            disabled={!server.joinable || disabled}
            onPick={() => onPick(server)}
          />
        ))}
        {list.rows === null && (
          <p className="server-list-empty">
            <LoaderCircle className="spin" size={14} />{" "}
            {t("launch.servers.loading")}
          </p>
        )}
        {list.rows !== null && visible.length === 0 && (
          <p className="server-list-empty">
            {t(
              list.listing ? "launch.servers.loading" : "launch.servers.empty",
            )}
          </p>
        )}
      </div>
      {list.rows !== null && list.hasMore && (
        <Button
          className="server-more"
          variant="outline"
          size="sm"
          disabled={list.listing}
          onClick={list.loadMore}
        >
          {list.listing && <LoaderCircle className="spin" />}
          {t("launch.servers.more")}
        </Button>
      )}
    </div>
  );
}

function ServerOption({
  server,
  checked,
  disabled,
  onPick,
}: {
  server: ServerRow;
  checked: boolean;
  disabled: boolean;
  onPick: () => void;
}) {
  const t = useT();
  const fill = server.maxPlayers
    ? Math.min(1, server.playing / server.maxPlayers)
    : 0;
  const name = server.name ?? t("launch.server.privateFallbackName");
  const stats = [
    server.ping != null && t("launch.servers.ping", { ms: server.ping }),
    server.fps != null &&
      t("launch.servers.fps", { fps: Math.round(server.fps) }),
    !server.joinable && t("launch.servers.full"),
  ].filter(Boolean);
  return (
    <button
      role="radio"
      aria-checked={checked}
      aria-label={
        server.kind === "vip"
          ? t("launch.servers.privateRow", { name })
          : t("launch.servers.publicRow", { id: server.id })
      }
      className={`server-option ${checked ? "active" : ""}`}
      disabled={disabled}
      onClick={onPick}
    >
      <span className="server-radio">{checked && <Check size={12} />}</span>
      <span className="server-players">
        <span className="tabular">
          {server.playing}/{server.maxPlayers}
        </span>
        <span className="server-fill">
          <span style={{ width: `${fill * 100}%` }} />
        </span>
      </span>
      <span className="server-name">
        {server.kind === "vip" ? (
          <>
            <Lock size={12} />
            {name}
          </>
        ) : (
          <code title={server.id}>{server.id}</code>
        )}
      </span>
      <span className="server-meta tabular">{stats.join(" · ")}</span>
    </button>
  );
}
