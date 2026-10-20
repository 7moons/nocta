import { useState } from "react";
import { ChevronDown, Ellipsis, FolderMinus, Pencil, Plus } from "lucide-react";
import {
  MenuContent,
  MenuItem,
  MenuRoot,
  MenuTrigger,
} from "@/components/ui/menu";
import { useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import type { DropHandlers } from "./FolderRow";
import { InlineName } from "./InlineName";
import type { FolderActions } from "./useFolderActions";

export function GroupHeader({
  name,
  count,
  open,
  dropping,
  drop,
  actions,
  onToggle,
  onNewFolder,
}: {
  name: string;
  count: number;
  open: boolean;
  dropping: boolean;
  drop: DropHandlers;
  actions: FolderActions;
  onToggle: () => void;
  onNewFolder: () => void;
}) {
  const t = useT();
  const format = useFormat();
  const [menu, setMenu] = useState(false);
  const [renaming, setRenaming] = useState(false);
  return (
    <MenuRoot open={menu} onOpenChange={setMenu}>
      <div
        className={`folder-group-header ${dropping ? "drop-into" : ""}`}
        onContextMenu={(event) => {
          event.preventDefault();
          setMenu(true);
        }}
        {...drop}
      >
        <button
          className="folder-group-toggle"
          aria-expanded={open}
          onClick={onToggle}
          onDoubleClick={() => setRenaming(true)}
        >
          <ChevronDown size={12} />
          {renaming ? (
            <InlineName
              initial={name}
              label={t("folders.renameGroup")}
              onSubmit={(next) => {
                setRenaming(false);
                void actions.renameGroup(name, next);
              }}
              onCancel={() => setRenaming(false)}
            />
          ) : (
            <span>{name}</span>
          )}
          <small>{format.number(count)}</small>
        </button>
        <button
          className="folder-menu-button"
          aria-label={t("folders.newInGroup", { name })}
          title={t("folders.newInGroup", { name })}
          onClick={onNewFolder}
        >
          <Plus size={13} />
        </button>
        <MenuTrigger
          className="folder-menu-button"
          aria-label={t("folders.groupMenu", { name })}
        >
          <Ellipsis size={13} />
        </MenuTrigger>
      </div>
      <MenuContent onCloseAutoFocus={(event) => event.preventDefault()}>
        <MenuItem icon={<Plus size={13} />} onSelect={onNewFolder}>
          {t("folders.new")}
        </MenuItem>
        <MenuItem
          icon={<Pencil size={13} />}
          onSelect={() => setRenaming(true)}
        >
          {t("folders.renameGroup")}
        </MenuItem>
        <MenuItem
          icon={<FolderMinus size={13} />}
          onSelect={() => void actions.renameGroup(name, null)}
        >
          {t("folders.ungroup")}
        </MenuItem>
      </MenuContent>
    </MenuRoot>
  );
}
