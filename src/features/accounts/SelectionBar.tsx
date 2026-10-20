import { useState } from "react";
import { Clipboard, RotateCcw, Trash2, X } from "lucide-react";
import type { ExportFormat, Folder } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import {
  SESSION_ACTIONS,
  type RestoreDestination,
  type SessionAction,
} from "./accountActions";
import {
  clipboardFormatStore,
  formatName,
  useClipboardFormats,
} from "./clipboardFormats";
import type { BulkTool } from "./BulkRobloxDialogs";

const EXPORT_FORMATS: ExportFormat[] = ["csv", "json"];

/** Actions for the selected accounts; Trash offers restore and permanent deletion instead. */
export function SelectionBar({
  count,
  trash,
  folders,
  sessionsReady,
  onSession,
  onRobux,
  onRoblox,
  onMove,
  onCopy,
  onExport,
  onRestore,
  onRemove,
  onClear,
}: {
  count: number;
  trash: boolean;
  folders: Folder[];
  sessionsReady: boolean;
  onSession: (kind: SessionAction) => void;
  onRobux: () => void;
  onRoblox: (tool: BulkTool) => void;
  onMove: (folderId: number | null) => void;
  onCopy: () => void;
  onExport: (format: ExportFormat) => void;
  onRestore: (destination: RestoreDestination) => void;
  /** Moves to Trash, or deletes permanently when already in Trash. */
  onRemove: () => void;
  onClear: () => void;
}) {
  const t = useT();
  const clipboard = useClipboardFormats();
  const [destination, setDestination] =
    useState<RestoreDestination>("original");
  const folderOptions = folders.map((folder) => (
    <option key={folder.id} value={folder.id}>
      {folder.name}
    </option>
  ));
  return (
    <div className="bulk-actions">
      <span>{t("accounts.selection.count", { count })}</span>
      {trash ? (
        <>
          <select
            aria-label={t("accounts.selection.restoreTo")}
            value={destination}
            onChange={(event) =>
              setDestination(event.target.value as RestoreDestination)
            }
          >
            <option value="original">{t("accounts.folders.original")}</option>
            <option value="general">{t("accounts.folders.general")}</option>
            {folderOptions}
          </select>
          <Button
            variant="outline"
            size="sm"
            onClick={() => onRestore(destination)}
          >
            <RotateCcw />
            {t("accounts.selection.restore")}
          </Button>
          <Button variant="destructive" size="sm" onClick={onRemove}>
            <Trash2 />
            {t("accounts.selection.deleteForever")}
          </Button>
        </>
      ) : (
        <>
          <select
            aria-label={t("accounts.selection.sessionActions")}
            value=""
            disabled={!sessionsReady}
            onChange={(event) => {
              if (event.target.value === "robux") return onRobux();
              if (
                event.target.value === "privacy" ||
                event.target.value === "befriend"
              )
                return onRoblox(event.target.value);
              const kind = SESSION_ACTIONS.find(
                (item) => item === event.target.value,
              );
              if (kind) onSession(kind);
            }}
          >
            <option value="">{t("accounts.selection.sessionPrompt")}</option>
            {SESSION_ACTIONS.map((kind) => (
              <option key={kind} value={kind}>
                {t(`accounts.selection.session.${kind}`)}
              </option>
            ))}
            <option value="robux">{t("accounts.robux.check")}</option>
            <option value="privacy">{t("accounts.bulk.privacy.option")}</option>
            <option value="befriend">
              {t("accounts.bulk.befriend.option")}
            </option>
          </select>
          <select
            aria-label={t("accounts.selection.move")}
            value=""
            onChange={(event) => {
              const value = event.target.value;
              if (value) onMove(value === "general" ? null : Number(value));
            }}
          >
            <option value="">{t("accounts.selection.movePrompt")}</option>
            <option value="general">{t("accounts.folders.general")}</option>
            {folderOptions}
          </select>
          <select
            aria-label={t("accounts.selection.clipboardFormat")}
            value={clipboard.selected.id}
            onChange={(event) =>
              clipboardFormatStore.select(event.target.value)
            }
          >
            {clipboard.formats.map((format) => (
              <option key={format.id} value={format.id}>
                {formatName(format, t)}
              </option>
            ))}
          </select>
          <Button variant="outline" size="sm" onClick={onCopy}>
            <Clipboard />
            {t("accounts.selection.copy")}
          </Button>
          <select
            aria-label={t("accounts.export.label")}
            value=""
            onChange={(event) => {
              const format = EXPORT_FORMATS.find(
                (item) => item === event.target.value,
              );
              if (format) onExport(format);
            }}
          >
            <option value="">{t("accounts.export.prompt")}</option>
            {EXPORT_FORMATS.map((format) => (
              <option key={format} value={format}>
                {t(`accounts.export.${format}`)}
              </option>
            ))}
          </select>
          <Button variant="destructive" size="sm" onClick={onRemove}>
            <Trash2 />
            {t("accounts.selection.trash")}
          </Button>
        </>
      )}
      <Button
        className="ml-auto"
        variant="ghost"
        size="icon"
        aria-label={t("accounts.selection.clear")}
        onClick={onClear}
      >
        <X />
      </Button>
    </div>
  );
}
