import { useCallback, useEffect, useRef, useState } from "react";
import type { ServerRow } from "@/generated/contracts";
import { api } from "@/lib/ipc";
import { errorMessage } from "@/lib/utils";

export interface ServerQuery {
  placeId: string;
  /** Account whose private servers are listed too, or null for public servers only. */
  privateFor: number | null;
  mostPlayersFirst: boolean;
  hideFull: boolean;
}

/**
 * Public servers of a game, sorted by Roblox across the whole game (not one page), plus the
 * private servers of one account. Reloads whenever the query changes; `loadMore` appends the
 * next page. Requests that are no longer wanted are cancelled in the backend.
 */
export function useServerList(
  query: ServerQuery,
  onError: (message: string) => void,
) {
  const [rows, setRows] = useState<ServerRow[] | null>(null);
  const [warnings, setWarnings] = useState<string[]>([]);
  const [cursor, setCursor] = useState<string | null>(null);
  const [listing, setListing] = useState(false);
  const requestKey = useRef(`launch-servers-${crypto.randomUUID()}`);
  const version = useRef(0);
  const { placeId, privateFor, mostPlayersFirst, hideFull } = query;

  const load = useCallback(
    async (after: string | null) => {
      const current = ++version.current;
      setListing(true);
      onError("");
      try {
        const result = await api.listServers(
          requestKey.current,
          placeId,
          privateFor,
          privateFor !== null && after === null,
          { mostPlayersFirst, excludeFull: hideFull, cursor: after },
        );
        if (current !== version.current || result.cancelled) return;
        setRows((previous) =>
          after && previous ? [...previous, ...result.items] : result.items,
        );
        setWarnings(after ? [] : result.warnings);
        setCursor(result.nextCursor ?? null);
      } catch (failure) {
        if (current === version.current) {
          setRows((previous) => previous ?? []);
          onError(errorMessage(failure));
        }
      } finally {
        if (current === version.current) setListing(false);
      }
    },
    [placeId, privateFor, mostPlayersFirst, hideFull, onError],
  );

  useEffect(() => {
    setRows(null);
    setCursor(null);
    void load(null);
  }, [load]);
  useEffect(() => {
    const key = requestKey.current;
    const versions = version;
    return () => {
      versions.current++;
      void api.cancelServerList(key).catch(() => {});
    };
  }, []);

  return {
    rows,
    warnings,
    listing,
    hasMore: cursor !== null,
    refresh: () => void load(null),
    loadMore: () => void load(cursor),
  };
}
