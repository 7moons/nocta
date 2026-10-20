import { useState, type DragEvent } from "react";
import {
  Ellipsis,
  Folder as FolderIcon,
  FolderInput,
  FolderPlus,
  Palette,
  Pipette,
  Pencil,
  Trash2,
} from "lucide-react";
import type { Folder } from "@/generated/contracts";
import {
  MenuContent,
  MenuItem,
  MenuLabel,
  MenuRoot,
  MenuSeparator,
  MenuSub,
  MenuSubContent,
  MenuSubTrigger,
  MenuSwatch,
  MenuTrigger,
} from "@/components/ui/menu";
import { ColorDot } from "@/components/ui/ColorDot";
import { useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import { FolderColorDialog } from "./FolderColorDialog";
import { FOLDER_COLORS, sameColor } from "./folderTree";
import { FOLDER_DRAG, type DropPosition } from "./folderDrag";
import { InlineName } from "./InlineName";
import type { FolderActions } from "./useFolderActions";

export interface DropHandlers {
  onDragOver: (event: DragEvent<HTMLElement>) => void;
  onDragLeave: () => void;
  onDrop: (event: DragEvent<HTMLElement>) => void;
}

export function FolderRow({
  folder,
  count,
  active,
  groups,
  palette,
  onPalette,
  drop,
  dropAt,
  actions,
  onSelect,
}: {
  folder: Folder;
  count: number;
  active: boolean;
  /** Every group name, for "Move to group". */
  groups: string[];
  /** Colors the user saved. */
  palette: string[];
  onPalette: (colors: string[]) => void;
  drop: DropHandlers;
  dropAt: DropPosition | null;
  actions: FolderActions;
  onSelect: () => void;
}) {
  const t = useT();
  const format = useFormat();
  const [menu, setMenu] = useState(false);
  const [editing, setEditing] = useState<"name" | "group" | "color" | null>(
    null,
  );
  const remove = () => {
    if (window.confirm(t("folders.confirmDelete", { name: folder.name })))
      void actions.remove(folder);
  };

  return (
    <MenuRoot open={menu} onOpenChange={setMenu}>
      <div
        className={`folder-row ${active ? "active" : ""} ${dropAt ? `drop-${dropAt}` : ""}`}
        draggable={editing === null}
        onDragStart={(event) => {
          event.dataTransfer.setData(FOLDER_DRAG, String(folder.id));
          event.dataTransfer.effectAllowed = "move";
        }}
        onContextMenu={(event) => {
          event.preventDefault();
          setMenu(true);
        }}
        {...drop}
      >
        <button
          className="folder-select"
          title={`${folder.name} (${format.number(count)})`}
          aria-current={active ? "true" : undefined}
          onClick={onSelect}
          onDoubleClick={() => setEditing("name")}
          onKeyDown={(event) => {
            if (event.key === "F2") setEditing("name");
            if (event.key === "Delete") remove();
          }}
        >
          <FolderIcon size={14} style={{ color: folder.color }} />
          {editing === "name" ? (
            <InlineName
              initial={folder.name}
              label={t("folders.rename")}
              onSubmit={(name) => {
                setEditing(null);
                void actions.update(folder, { name });
              }}
              onCancel={() => setEditing(null)}
            />
          ) : (
            <span className="folder-name">{folder.name}</span>
          )}
          <small>{format.number(count)}</small>
        </button>
        <MenuTrigger
          className="folder-menu-button"
          aria-label={t("folders.menu", { name: folder.name })}
        >
          <Ellipsis size={13} />
        </MenuTrigger>
      </div>
      {editing === "color" && (
        <FolderColorDialog
          folder={folder}
          palette={palette}
          onPalette={onPalette}
          onClose={() => setEditing(null)}
          onSave={(color) => {
            setEditing(null);
            void actions.update(folder, { color });
          }}
        />
      )}
      {editing === "group" && (
        <div className="folder-row folder-row-input">
          <FolderPlus size={14} />
          <InlineName
            label={t("folders.newGroup")}
            placeholder={t("folders.groupPlaceholder")}
            onSubmit={(group) => {
              setEditing(null);
              void actions.move(folder.id, group, null);
            }}
            onCancel={() => setEditing(null)}
          />
        </div>
      )}
      {/* Focus stays on a field opened from the menu instead of going back to the button. */}
      <MenuContent onCloseAutoFocus={(event) => event.preventDefault()}>
        <MenuItem
          icon={<Pencil size={13} />}
          onSelect={() => setEditing("name")}
        >
          {t("folders.rename")}
        </MenuItem>
        <MenuSub>
          <MenuSubTrigger icon={<Palette size={13} />}>
            {t("folders.color")}
          </MenuSubTrigger>
          <MenuSubContent>
            {[
              { label: t("folders.colorPresets"), colors: FOLDER_COLORS },
              { label: t("folders.yourColors"), colors: palette },
            ]
              .filter((set) => set.colors.length > 0)
              .map((set) => (
                <div key={set.label} role="group" aria-label={set.label}>
                  <MenuLabel>{set.label}</MenuLabel>
                  <div className="menu-swatches">
                    {set.colors.map((color) => (
                      <MenuSwatch
                        key={color}
                        aria-label={color}
                        aria-checked={sameColor(folder.color, color)}
                        onSelect={() => void actions.update(folder, { color })}
                      >
                        <ColorDot
                          color={color}
                          chosen={sameColor(folder.color, color)}
                        />
                      </MenuSwatch>
                    ))}
                  </div>
                </div>
              ))}
            <MenuSeparator />
            <MenuItem
              icon={<Pipette size={13} />}
              onSelect={() => setEditing("color")}
            >
              {t("folders.customColor")}
            </MenuItem>
          </MenuSubContent>
        </MenuSub>
        <MenuSub>
          <MenuSubTrigger icon={<FolderInput size={13} />}>
            {t("folders.moveToGroup")}
          </MenuSubTrigger>
          <MenuSubContent>
            {folder.group && (
              <MenuItem
                onSelect={() => void actions.move(folder.id, null, null)}
              >
                {t("folders.noGroup")}
              </MenuItem>
            )}
            {groups
              .filter((group) => group !== folder.group)
              .map((group) => (
                <MenuItem
                  key={group}
                  onSelect={() => void actions.move(folder.id, group, null)}
                >
                  {group}
                </MenuItem>
              ))}
            <MenuSeparator />
            <MenuItem
              icon={<FolderPlus size={13} />}
              onSelect={() => setEditing("group")}
            >
              {t("folders.newGroup")}
            </MenuItem>
          </MenuSubContent>
        </MenuSub>
        <MenuSeparator />
        <MenuItem icon={<Trash2 size={13} />} danger onSelect={remove}>
          {t("folders.delete")}
        </MenuItem>
      </MenuContent>
    </MenuRoot>
  );
}
