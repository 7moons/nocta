import { useState } from "react";
import { Folder as FolderIcon, Plus, X } from "lucide-react";
import type { Folder } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { ColorDot } from "@/components/ui/ColorDot";
import { ColorPicker } from "@/components/ui/ColorPicker";
import { Dialog } from "@/components/ui/dialog";
import { useT } from "@/i18n";
import { isHex } from "@/lib/color";
import { FOLDER_COLORS, MAX_SAVED_COLORS, sameColor } from "./folderTree";

/**
 * Any color for a folder: the picker, a hex code, a preset or one of the user's saved colors,
 * with a preview. The color being picked can be added to the saved ones.
 */
export function FolderColorDialog({
  folder,
  palette,
  onPalette,
  onClose,
  onSave,
}: {
  folder: Folder;
  palette: string[];
  onPalette: (colors: string[]) => void;
  onClose: () => void;
  onSave: (color: string) => void;
}) {
  const t = useT();
  const [color, setColor] = useState(folder.color.toLowerCase());
  // What is typed in the hex field, which may be unfinished.
  const [typed, setTyped] = useState(color);
  const choose = (hex: string) => {
    setColor(hex.toLowerCase());
    setTyped(hex.toLowerCase());
  };
  const saved = palette.some((item) => sameColor(item, color));
  const full = palette.length >= MAX_SAVED_COLORS;

  const swatch = (item: string, remove?: () => void) => (
    <span key={item} className="color-choice">
      <button
        type="button"
        aria-label={item}
        aria-pressed={sameColor(item, color)}
        onClick={() => choose(item)}
      >
        <ColorDot color={item} chosen={sameColor(item, color)} size={22} />
      </button>
      {remove && (
        <button
          type="button"
          className="color-choice-remove"
          aria-label={t("folders.removeColor", { color: item })}
          title={t("folders.removeColor", { color: item })}
          onClick={remove}
        >
          <X size={10} strokeWidth={3} />
        </button>
      )}
    </span>
  );

  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      title={t("folders.colorTitle")}
      description={folder.name}
    >
      <form
        className="dialog-body folder-color"
        onSubmit={(event) => {
          event.preventDefault();
          onSave(color);
        }}
      >
        <ColorPicker
          value={color}
          onChange={choose}
          labels={{ area: t("folders.colorArea"), hue: t("folders.colorHue") }}
        />
        <div className="folder-color-row">
          <div className="folder-color-preview" aria-hidden="true">
            <FolderIcon size={16} style={{ color }} />
            <span>{folder.name}</span>
          </div>
          <label className="field-label folder-color-hex">
            {t("folders.colorHex")}
            <input
              value={typed}
              maxLength={7}
              spellCheck={false}
              onChange={(event) => {
                const value = event.target.value.trim();
                const hex = value.startsWith("#") ? value : `#${value}`;
                setTyped(value);
                if (isHex(hex)) setColor(hex.toLowerCase());
              }}
              onBlur={() => setTyped(color)}
            />
          </label>
        </div>
        <div
          className="color-set"
          role="group"
          aria-label={t("folders.colorPresets")}
        >
          <span className="color-set-label">{t("folders.colorPresets")}</span>
          <div className="color-set-swatches">
            {FOLDER_COLORS.map((item) => swatch(item))}
          </div>
        </div>
        <div
          className="color-set"
          role="group"
          aria-label={t("folders.yourColors")}
        >
          <span className="color-set-label">{t("folders.yourColors")}</span>
          <div className="color-set-swatches">
            {palette.map((item) =>
              swatch(item, () =>
                onPalette(palette.filter((other) => !sameColor(other, item))),
              ),
            )}
            <button
              type="button"
              className="color-add"
              disabled={saved || full}
              title={t(full ? "folders.paletteFull" : "folders.addColor")}
              onClick={() => onPalette([...palette, color])}
            >
              <Plus size={13} />
              {t("folders.addColor")}
            </button>
          </div>
        </div>
        <div className="dialog-footer">
          <Button variant="outline" type="button" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button>{t("folders.colorSave")}</Button>
        </div>
      </form>
    </Dialog>
  );
}
