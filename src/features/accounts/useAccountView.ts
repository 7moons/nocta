import { useMemo, useState } from "react";
import type { RowSelectionState } from "@tanstack/react-table";
import type { Account } from "@/generated/contracts";
import { ownOrder, pinnedFirst } from "./accountDetails";
import { searchAccounts } from "./search";

export type FolderFilter = number | null | "all";

/**
 * The accounts shown in the table and which of them are selected. Selection only covers
 * visible rows and is cleared whenever the view changes, so actions never reach hidden ones.
 */
export function useAccountView(
  accounts: Account[],
  folder: FolderFilter,
  trash: boolean,
  separators: string,
  /** The session-state filter, kept in the navigation history so Back restores it. */
  health: string,
  setHealth: (health: string) => void,
) {
  const [search, setSearch] = useState("");
  const [selection, setSelection] = useState<RowSelectionState>({});
  const [orderedIds, setOrderedIds] = useState<number[]>([]);
  // Changing what is shown starts a new selection (state adjusted while rendering, not in an effect).
  const viewKey = `${String(folder)}|${trash}|${search}|${health}`;
  const [selectionFor, setSelectionFor] = useState(viewKey);
  if (selectionFor !== viewKey) {
    setSelectionFor(viewKey);
    setSelection({});
  }

  const inScope = useMemo(
    () =>
      accounts.filter((account) =>
        trash
          ? !!account.deletedAt
          : !account.deletedAt &&
            (folder === "all" || account.folderId === folder),
      ),
    [accounts, folder, trash],
  );
  const shown = useMemo(
    () =>
      pinnedFirst(
        searchAccounts(
          ownOrder(inScope).filter(
            (account) => health === "all" || account.health === health,
          ),
          search,
          separators,
        ),
      ),
    [inScope, search, health, separators],
  );
  const shownIds = useMemo(
    () => new Set(shown.map((account) => account.id)),
    [shown],
  );
  // Selected IDs in table order, so bulk work runs top to bottom.
  const selected = orderedIds.filter(
    (id) => selection[String(id)] && shownIds.has(id),
  );

  return {
    search,
    setSearch,
    health,
    setHealth,
    selection,
    setSelection,
    setOrderedIds,
    /** Accounts in the folder (or Trash) before search and status filters. */
    inScope,
    shown,
    selected,
    selectOnly: (id: number) => setSelection({ [id]: true }),
    clearSelection: () => setSelection({}),
  };
}
