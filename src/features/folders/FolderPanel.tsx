import { useMemo, useState, type DragEvent } from "react";
import {
  Folder as FolderIcon,
  FolderOpen,
  FolderPlus,
  Plus,
} from "lucide-react";
import type { Account, Folder } from "@/generated/contracts";
import type { FolderSelection } from "@/app/navigation";
import { Button } from "@/components/ui/button";
import { useUiPreference } from "@/hooks/useUiPreference";
import { useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import type { RunAction } from "@/lib/actions";
import {
  ACCOUNTS_DRAG,
  FOLDER_DRAG,
  carries,
  draggedAccounts,
  draggedFolder,
  halfOf,
  type DropPosition,
} from "./folderDrag";
import { countAccounts, layoutFolders } from "./folderTree";
import { FolderRow, type DropHandlers } from "./FolderRow";
import { GroupHeader } from "./GroupHeader";
import { InlineName } from "./InlineName";
import { useFolderActions } from "./useFolderActions";

interface DropTarget {
  key: string;
  at: DropPosition;
}

/**
 * The folders of the accounts view: select one, create and rename in place, change color and
 * group from the menu (right click or ⋯), drag folders to reorder them and accounts onto a
 * folder to move them.
 */
export function FolderPanel({
  accounts,
  folders: saved,
  selected,
  collapsed,
  palette,
  defaultColor,
  run,
  onSelect,
  onMoveAccounts,
  onPalette,
}: {
  accounts: Account[];
  folders: Folder[];
  /** The selected folder, or undefined outside the accounts view. */
  selected: FolderSelection | undefined;
  /** The sidebar shows icons only. */
  collapsed: boolean;
  /** Colors the user saved for folders. */
  palette: string[];
  /** The color new folders get. */
  defaultColor: string;
  run: RunAction;
  onSelect: (folder: FolderSelection) => void;
  onMoveAccounts: (ids: number[], folder: number | null) => void;
  onPalette: (colors: string[]) => void;
}) {
  const t = useT();
  const format = useFormat();
  const actions = useFolderActions(saved, run);
  const { ungrouped, groups } = useMemo(
    () => layoutFolders(actions.folders),
    [actions.folders],
  );
  const counts = useMemo(() => countAccounts(accounts), [accounts]);
  const total = useMemo(
    () => [...counts.values()].reduce((sum, count) => sum + count, 0),
    [counts],
  );
  const [closedGroups, setClosedGroups] = useUiPreference<string[]>(
    "collapsedFolderGroups",
    [],
  );
  const [creating, setCreating] = useState<{ group: string | null } | null>(
    null,
  );
  const [target, setTarget] = useState<DropTarget | null>(null);
  const groupNames = groups.map((group) => group.name);

  /** Drop handling for one row: what it accepts and what a drop does. */
  const dropOn = (
    key: string,
    accepts: { folders?: "between" | "into"; accounts?: boolean },
    onFolder: (id: number, at: DropPosition) => void,
    onAccounts: (ids: number[]) => void,
  ): DropHandlers => ({
    onDragOver: (event: DragEvent<HTMLElement>) => {
      let at: DropPosition | null = null;
      if (accepts.folders && carries(event, FOLDER_DRAG))
        at = accepts.folders === "into" ? "into" : halfOf(event);
      else if (accepts.accounts && carries(event, ACCOUNTS_DRAG)) at = "into";
      if (!at) return;
      event.preventDefault();
      event.dataTransfer.dropEffect = "move";
      if (target?.key !== key || target.at !== at) setTarget({ key, at });
    },
    onDragLeave: () =>
      setTarget((current) => (current?.key === key ? null : current)),
    onDrop: (event: DragEvent<HTMLElement>) => {
      event.preventDefault();
      const at = target?.key === key ? target.at : "into";
      setTarget(null);
      const folder = draggedFolder(event);
      if (folder !== null) onFolder(folder, at);
      else {
        const ids = draggedAccounts(event);
        if (ids.length) onAccounts(ids);
      }
    },
  });

  const folderDrop = (folder: Folder, list: Folder[]) =>
    dropOn(
      `folder:${folder.id}`,
      { folders: "between", accounts: true },
      (id, at) => {
        if (id === folder.id) return;
        const index = list.findIndex((item) => item.id === folder.id);
        const next = list.slice(index + 1).find((item) => item.id !== id);
        const before = at === "before" ? folder.id : (next?.id ?? null);
        void actions.move(id, folder.group ?? null, before);
      },
      (ids) => onMoveAccounts(ids, folder.id),
    );
  const dropAt = (key: string) => (target?.key === key ? target.at : null);

  const row = (folder: Folder, list: Folder[]) => (
    <FolderRow
      key={folder.id}
      folder={folder}
      count={counts.get(folder.id) ?? 0}
      active={selected === folder.id}
      groups={groupNames}
      palette={palette}
      onPalette={onPalette}
      drop={folderDrop(folder, list)}
      dropAt={dropAt(`folder:${folder.id}`)}
      actions={actions}
      onSelect={() => onSelect(folder.id)}
    />
  );
  const newFolderInput = (group: string | null) =>
    creating && creating.group === group ? (
      <div className="folder-row folder-row-input">
        <FolderPlus size={14} />
        <InlineName
          label={t("folders.new")}
          placeholder={t("folders.namePlaceholder")}
          onSubmit={(name) => {
            setCreating(null);
            void actions.create(name, group, defaultColor);
          }}
          onCancel={() => setCreating(null)}
        />
      </div>
    ) : null;
  const special = (
    key: FolderSelection,
    icon: typeof FolderIcon,
    label: string,
    count: number,
    drop?: DropHandlers,
  ) => {
    const Icon = icon;
    const dropKey = `special:${String(key)}`;
    return (
      <div
        className={`folder-row ${selected === key ? "active" : ""} ${dropAt(dropKey) ? "drop-into" : ""}`}
        {...drop}
      >
        <button
          className="folder-select"
          title={`${label} (${format.number(count)})`}
          aria-current={selected === key ? "true" : undefined}
          onClick={() => onSelect(key)}
        >
          <Icon size={14} />
          <span className="folder-name">{label}</span>
          <small>{format.number(count)}</small>
        </button>
      </div>
    );
  };

  return (
    <section className="folder-panel" aria-label={t("sidebar.folders")}>
      <div className="sidebar-section-heading">
        <span>{t("sidebar.folders")}</span>
        <Button
          variant="ghost"
          size="icon"
          aria-label={t("folders.new")}
          title={t("folders.new")}
          onClick={() => setCreating({ group: null })}
        >
          <Plus size={13} />
        </Button>
      </div>
      <div className="folder-list">
        {special("all", FolderOpen, t("accounts.folders.all"), total)}
        {special(
          null,
          FolderIcon,
          t("accounts.folders.general"),
          counts.get(null) ?? 0,
          dropOn(
            "special:null",
            { accounts: true },
            () => undefined,
            (ids) => onMoveAccounts(ids, null),
          ),
        )}
        {ungrouped.length > 0 && <div className="folder-divider" />}
        {ungrouped.map((folder) => row(folder, ungrouped))}
        {newFolderInput(null)}
        {groups.map((group) => {
          const open = collapsed || !closedGroups.includes(group.name);
          const count = group.folders.reduce(
            (sum, folder) => sum + (counts.get(folder.id) ?? 0),
            0,
          );
          return (
            <div key={group.name} className="folder-group">
              <GroupHeader
                name={group.name}
                count={count}
                open={open}
                dropping={dropAt(`group:${group.name}`) !== null}
                drop={dropOn(
                  `group:${group.name}`,
                  { folders: "into" },
                  (id) => void actions.move(id, group.name, null),
                  () => undefined,
                )}
                actions={actions}
                onToggle={() =>
                  setClosedGroups(
                    open
                      ? [...closedGroups, group.name]
                      : closedGroups.filter((name) => name !== group.name),
                  )
                }
                onNewFolder={() => {
                  if (!open)
                    setClosedGroups(
                      closedGroups.filter((name) => name !== group.name),
                    );
                  setCreating({ group: group.name });
                }}
              />
              {open &&
                group.folders.map((folder) => row(folder, group.folders))}
              {open && newFolderInput(group.name)}
            </div>
          );
        })}
        {actions.folders.length === 0 && !creating && (
          <p className="sidebar-hint">{t("folders.empty")}</p>
        )}
      </div>
    </section>
  );
}
