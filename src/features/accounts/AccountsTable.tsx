import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type CSSProperties,
  type Dispatch,
  type SetStateAction,
} from "react";
import { ArrowDown, ArrowUp, ArrowUpDown, Users } from "lucide-react";
import {
  flexRender,
  getCoreRowModel,
  getSortedRowModel,
  useReactTable,
  type RowSelectionState,
  type SortingState,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { Account, AccountTask, Folder } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { useAccountColumns, type RowAction } from "./accountColumns";
import type { QuickActionId } from "./quickActions";
import { ACCOUNTS_DRAG, setDragImage } from "@/features/folders/folderDrag";
import { matchRank, searchTerms } from "./search";

export type { RowAction } from "./accountColumns";

const ROW_HEIGHT = 48;
const COMPACT_ROW_HEIGHT = 32;

/**
 * The virtualized account table. With several search terms, rows are grouped by the first
 * term they match, so results follow the order the terms were typed in.
 */
export function AccountsTable({
  accounts,
  tasks,
  folders,
  selection,
  onSelection,
  onEdit,
  onActions,
  rowActions,
  onRowAction,
  onOrder,
  onReorder,
  onAdd,
  trash,
  loading,
  search,
  separators,
  compact = false,
}: {
  accounts: Account[];
  tasks: AccountTask[];
  folders: Folder[];
  selection: RowSelectionState;
  onSelection: Dispatch<SetStateAction<RowSelectionState>>;
  onEdit: (account: Account) => void;
  onActions: (account: Account) => void;
  rowActions: RowAction[];
  onRowAction: (id: QuickActionId, account: Account) => void;
  onOrder: (ids: number[]) => void;
  /** Drops `moved` onto `target` in the user's own order; absent where it cannot apply. */
  onReorder?: (moved: number, target: number) => void;
  onAdd: () => void;
  trash: boolean;
  loading: boolean;
  search: string;
  separators: string;
  /** One-line rows, so more accounts fit on screen. */
  compact?: boolean;
}) {
  const t = useT();
  const [sorting, setSorting] = useState<SortingState>([]);
  const columns = useAccountColumns({
    tasks,
    folders,
    rowActions,
    trash,
    onRowAction,
    onActions,
  });
  const table = useReactTable({
    data: accounts,
    columns,
    state: {
      sorting:
        sorting.length && sorting[0].id !== "id"
          ? [...sorting, { id: "id", desc: false }]
          : sorting,
      rowSelection: selection,
    },
    onSortingChange: setSorting,
    onRowSelectionChange: onSelection,
    getCoreRowModel: getCoreRowModel(),
    getSortedRowModel: getSortedRowModel(),
    getRowId: (row) => String(row.id),
    enableRowSelection: true,
  });
  const sortedRows = table.getRowModel().rows;
  const terms = useMemo(
    () => searchTerms(search, separators),
    [search, separators],
  );
  const rows = useMemo(() => {
    if (terms.length < 2) return sortedRows;
    const groups = terms.map(() => [] as typeof sortedRows);
    for (const row of sortedRows) {
      const rank = matchRank(row.original, terms);
      if (rank !== -1) groups[rank].push(row);
    }
    return groups.flat();
  }, [sortedRows, terms]);
  useEffect(() => onOrder(rows.map((row) => row.original.id)), [rows, onOrder]);
  // Rows can be dragged into the user's own order while no column sorting or search applies.
  const draggable = !!onReorder && sorting.length === 0 && terms.length === 0;
  const [dragged, setDragged] = useState<number | null>(null);
  const [dropTarget, setDropTarget] = useState<number | null>(null);
  const scroller = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => (compact ? COMPACT_ROW_HEIGHT : ROW_HEIGHT),
    overscan: 8,
    getItemKey: (index) => rows[index].id,
  });
  // Row height changes with the compact setting; measure the rows again.
  useEffect(() => virtualizer.measure(), [compact, virtualizer]);
  return (
    <div
      className={`accounts-table ${compact ? "compact" : ""}`}
      style={
        {
          "--row-actions-width": `${(rowActions.length + 1) * 28}px`,
        } as CSSProperties
      }
      role="table"
      aria-label={t(
        trash ? "accounts.table.labelTrash" : "accounts.table.label",
      )}
      aria-rowcount={rows.length + 1}
    >
      <div className="table-grid table-header" role="row">
        {table.getHeaderGroups()[0].headers.map((header) => (
          <div key={header.id} role="columnheader">
            {header.column.getCanSort() ? (
              <button
                className="sort-button"
                onClick={header.column.getToggleSortingHandler()}
              >
                {flexRender(
                  header.column.columnDef.header,
                  header.getContext(),
                )}
                {header.column.getIsSorted() === "asc" ? (
                  <ArrowUp />
                ) : header.column.getIsSorted() === "desc" ? (
                  <ArrowDown />
                ) : (
                  <ArrowUpDown />
                )}
              </button>
            ) : (
              flexRender(header.column.columnDef.header, header.getContext())
            )}
          </div>
        ))}
      </div>
      <div ref={scroller} className="table-scroll">
        {rows.length === 0 ? (
          <div className="empty-state">
            <span className="empty-icon">
              <Users size={25} />
            </span>
            <h3>
              {t(
                loading
                  ? "accounts.empty.loading"
                  : trash
                    ? "accounts.empty.trashTitle"
                    : "accounts.empty.title",
              )}
            </h3>
            <p>
              {t(
                trash ? "accounts.empty.trashDetail" : "accounts.empty.detail",
              )}
            </p>
            {!trash && !loading && (
              <Button variant="outline" onClick={onAdd}>
                {t("accounts.add")}
              </Button>
            )}
          </div>
        ) : (
          <div
            className="virtual-space"
            style={{ height: virtualizer.getTotalSize() }}
          >
            {virtualizer.getVirtualItems().map((item) => {
              const row = rows[item.index];
              return (
                <div
                  key={row.id}
                  role="row"
                  aria-rowindex={item.index + 2}
                  className={`table-grid table-row ${row.getIsSelected() ? "selected" : ""} ${dropTarget === row.original.id ? "drop-target" : ""} ${dragged === row.original.id ? "dragging" : ""}`}
                  draggable={!trash}
                  onDragStart={(event) => {
                    // A selected row carries the whole selection onto a folder.
                    const ids = row.getIsSelected()
                      ? table
                          .getSelectedRowModel()
                          .rows.map((selected) => selected.original.id)
                      : [row.original.id];
                    event.dataTransfer.effectAllowed = "move";
                    event.dataTransfer.setData(
                      ACCOUNTS_DRAG,
                      JSON.stringify(ids),
                    );
                    setDragImage(
                      event,
                      t("accounts.count", { count: ids.length }),
                    );
                    if (draggable) setDragged(row.original.id);
                  }}
                  onDragOver={(event) => {
                    if (dragged === null) return;
                    event.preventDefault();
                    setDropTarget(row.original.id);
                  }}
                  onDragLeave={() =>
                    setDropTarget((target) =>
                      target === row.original.id ? null : target,
                    )
                  }
                  onDrop={(event) => {
                    event.preventDefault();
                    if (dragged !== null && dragged !== row.original.id)
                      onReorder?.(dragged, row.original.id);
                    setDragged(null);
                    setDropTarget(null);
                  }}
                  onDragEnd={() => {
                    setDragged(null);
                    setDropTarget(null);
                  }}
                  style={{
                    transform: `translateY(${item.start}px)`,
                    height: item.size,
                  }}
                  onDoubleClick={() => onEdit(row.original)}
                  onContextMenu={(event) => {
                    event.preventDefault();
                    onActions(row.original);
                  }}
                >
                  {row.getVisibleCells().map((cell) => (
                    <div key={cell.id} role="cell">
                      {flexRender(
                        cell.column.columnDef.cell,
                        cell.getContext(),
                      )}
                    </div>
                  ))}
                </div>
              );
            })}
          </div>
        )}
      </div>
      <div className="table-footer">
        <span>{t("accounts.count", { count: accounts.length })}</span>
        <span>
          {t("accounts.selection.count", {
            count: Object.values(selection).filter(Boolean).length,
          })}
        </span>
      </div>
    </div>
  );
}
