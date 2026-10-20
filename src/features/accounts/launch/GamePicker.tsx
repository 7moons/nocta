import { useEffect, useRef, useState } from "react";
import {
  Bookmark,
  BookmarkCheck,
  Link2,
  LoaderCircle,
  Search,
  X,
} from "lucide-react";
import type { AppConfig, PlaceInfo } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { GameIcon, GameThumb, PlaceName } from "./GameIcon";
import { GameSearch } from "./GameSearch";
import { classifyGameQuery } from "./target";
import { SAVED_GAMES_LIMIT, type useTargetEditor } from "./useTargetEditor";

/** Step 1 of the launch panel: the selected game and the ways to change it. */
export function GamePicker({
  config,
  game,
  editor,
}: {
  config: AppConfig;
  game: PlaceInfo | null;
  editor: ReturnType<typeof useTargetEditor>;
}) {
  const t = useT();
  const placeId = config.placeId;
  const [query, setQuery] = useState("");
  const searchRef = useRef<HTMLInputElement>(null);
  const parsed = classifyGameQuery(query);
  const saved = config.presets.some((item) => item.placeId === placeId);
  const recent = editor.recent.filter(
    (id) => !config.presets.some((item) => item.placeId === id),
  );
  const gameName =
    game?.name ?? t("launch.game.placeFallback", { id: placeId });

  // Without a game, typing one is the only thing to do.
  const focusOnOpen = useRef(!placeId);
  useEffect(() => {
    if (focusOnOpen.current) searchRef.current?.focus();
  }, []);

  const choose = (id: string, info?: PlaceInfo) =>
    void editor.chooseGame(id, info).then((done) => done && setQuery(""));
  const applyLink = (url: string) =>
    void editor.applyLink(url).then((done) => done && setQuery(""));

  return (
    <section className="launch-step" aria-label={t("launch.game.title")}>
      <h3>
        <span className="launch-step-number">1</span>
        {t("launch.game.title")}
      </h3>
      {placeId && (
        <div className="launch-game-card">
          <GameThumb src={game?.iconUrl} alt={gameName} size={20} />
          <div>
            <strong>{gameName}</strong>
            <small>{t("launch.game.placeFallback", { id: placeId })}</small>
          </div>
          <Button
            variant="ghost"
            size="icon"
            aria-label={t(saved ? "launch.game.unsave" : "launch.game.save")}
            title={t(saved ? "launch.game.unsave" : "launch.game.save")}
            disabled={!saved && config.presets.length >= SAVED_GAMES_LIMIT}
            onClick={() => editor.toggleSaved(gameName)}
          >
            {saved ? <BookmarkCheck /> : <Bookmark />}
          </Button>
        </div>
      )}
      <label className="search-field launch-search">
        {editor.busy ? (
          <LoaderCircle className="spin" size={14} />
        ) : (
          <Search size={14} />
        )}
        <input
          ref={searchRef}
          aria-label={t("launch.game.find")}
          placeholder={t(
            placeId
              ? "launch.game.findPlaceholderChange"
              : "launch.game.findPlaceholder",
          )}
          value={query}
          disabled={editor.busy}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key !== "Enter") return;
            if (parsed.kind === "place") choose(parsed.placeId);
            else if (parsed.kind === "link") applyLink(parsed.url);
          }}
        />
        {query && (
          <button
            aria-label={t("launch.game.clearSearch")}
            onClick={() => setQuery("")}
          >
            <X size={12} />
          </button>
        )}
      </label>
      {parsed.kind === "search" && (
        <GameSearch
          text={parsed.text}
          current={placeId}
          onPick={(result) =>
            choose(result.placeId, {
              placeId: result.placeId,
              name: result.name,
              universeId: result.universeId,
              iconUrl: result.iconUrl,
            })
          }
        />
      )}
      {parsed.kind === "place" && (
        <button className="game-option" onClick={() => choose(parsed.placeId)}>
          <GameIcon placeId={parsed.placeId} />
          <span>
            <strong>
              <PlaceName placeId={parsed.placeId} />
            </strong>
            <small>{t("launch.game.placeHint", { id: parsed.placeId })}</small>
          </span>
        </button>
      )}
      {parsed.kind === "link" && (
        <button className="game-option" onClick={() => applyLink(parsed.url)}>
          <span className="game-icon-fallback">
            <Link2 size={18} />
          </span>
          <span>
            <strong>{t("launch.game.useLink")}</strong>
            <small>{t("launch.game.useLinkHint")}</small>
          </span>
        </button>
      )}
      {parsed.kind === "invalid" && (
        <p className="launch-hint">{t("launch.game.invalidLink")}</p>
      )}
      {parsed.kind === "empty" &&
        (config.presets.length > 0 || recent.length > 0) && (
          <div className="game-shelves">
            {config.presets.length > 0 && (
              <GameShelf
                label={t("launch.game.saved")}
                listLabel={t("launch.game.savedList")}
                games={config.presets}
                current={placeId}
                onPick={choose}
              />
            )}
            {recent.length > 0 && (
              <GameShelf
                label={t("launch.game.recent")}
                listLabel={t("launch.game.recentList")}
                games={recent.map((id) => ({ placeId: id }))}
                current={placeId}
                onPick={choose}
              />
            )}
          </div>
        )}
      {editor.warning && (
        <p className="launch-hint warning">{editor.warning}</p>
      )}
      {editor.error && (
        <p className="form-error" role="alert">
          {editor.error}
        </p>
      )}
    </section>
  );
}

function GameShelf({
  label,
  listLabel,
  games,
  current,
  onPick,
}: {
  label: string;
  listLabel: string;
  games: { placeId: string; name?: string }[];
  current: string;
  onPick: (placeId: string) => void;
}) {
  return (
    <div className="game-shelf">
      <span>{label}</span>
      <div role="list" aria-label={listLabel}>
        {games.map((item) => (
          <button
            key={item.placeId}
            role="listitem"
            className={`game-chip ${item.placeId === current ? "active" : ""}`}
            onClick={() => onPick(item.placeId)}
          >
            <GameIcon placeId={item.placeId} />
            <span>{item.name ?? <PlaceName placeId={item.placeId} />}</span>
          </button>
        ))}
      </div>
    </div>
  );
}
