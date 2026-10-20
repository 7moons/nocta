import { useEffect, useState } from "react";
import { Check, LoaderCircle } from "lucide-react";
import type { GameSearchResult } from "@/generated/contracts";
import { useLanguage, useT } from "@/i18n";
import { api } from "@/lib/ipc";
import { errorMessage } from "@/lib/utils";
import { GameThumb } from "./GameIcon";

// Waiting for a pause in typing makes a name cost one request rather than one per key.
const SEARCH_DELAY_MS = 400;

type SearchState =
  | { text: string; status: "loading" }
  | { text: string; status: "done"; results: GameSearchResult[] }
  | { text: string; status: "failed"; message: string };

/** Roblox games matching `text`, searched as the user types. */
export function GameSearch({
  text,
  current,
  onPick,
}: {
  text: string;
  current: string;
  onPick: (result: GameSearchResult) => void;
}) {
  const t = useT();
  const language = useLanguage();
  const [search, setSearch] = useState<SearchState | null>(null);
  useEffect(() => {
    let active = true;
    const timer = setTimeout(() => {
      api
        .searchGames(text)
        .then((results) => {
          if (active) setSearch({ text, status: "done", results });
        })
        .catch((failure) => {
          if (active)
            setSearch({
              text,
              status: "failed",
              message: errorMessage(failure),
            });
        });
    }, SEARCH_DELAY_MS);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [text]);
  // Results of an older query never show for the current one.
  const state: SearchState =
    search?.text === text ? search : { text, status: "loading" };

  if (state.status === "failed")
    return <p className="launch-hint warning">{state.message}</p>;
  if (state.status === "loading")
    return (
      <p className="launch-hint">
        <LoaderCircle className="spin" size={13} /> {t("launch.search.loading")}
      </p>
    );
  if (state.results.length === 0)
    return <p className="launch-hint">{t("launch.search.empty")}</p>;
  const players = new Intl.NumberFormat(language, {
    notation: "compact",
    maximumFractionDigits: 1,
  });
  return (
    <div
      className="game-results"
      role="list"
      aria-label={t("launch.search.results")}
    >
      {state.results.map((result) => (
        <button
          key={result.placeId}
          role="listitem"
          className={`game-option ${result.placeId === current ? "active" : ""}`}
          onClick={() => onPick(result)}
        >
          <GameThumb src={result.iconUrl} size={18} />
          <span>
            <strong>{result.name}</strong>
            <small>
              {result.creator
                ? t("launch.search.byCreator", {
                    creator: result.creator,
                    players: players.format(result.playing),
                  })
                : t("launch.search.playing", {
                    players: players.format(result.playing),
                  })}
            </small>
          </span>
          {result.placeId === current && <Check size={15} />}
        </button>
      ))}
    </div>
  );
}
