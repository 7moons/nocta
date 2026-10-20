import { useEffect, useState } from "react";
import {
  Check,
  Copy,
  Import as ImportIcon,
  Palette,
  Pencil,
  Plus,
  Share,
  Trash2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { defaultThemeFor } from "@/theme/presets";
import { themeStore, useThemes } from "@/theme/store";
import { exportTheme, newThemeId, type Theme } from "@/theme/theme";
import { SettingSection } from "../SettingControls";
import { ThemeImportDialog } from "./ThemeImportDialog";
import { ThemePreview } from "./ThemePreview";

const NAME_LIMIT = 40;
const EXPORT_NOTICE_MS = 2500;

/** The theme gallery and actions on the active theme. Editing opens through `onEdit`. */
export function ThemeSettings({ onEdit }: { onEdit: (theme: Theme) => void }) {
  const t = useT();
  const { themes, active } = useThemes();
  const [importing, setImporting] = useState(false);
  const [exported, setExported] = useState<"copied" | "failed" | null>(null);
  useEffect(() => {
    if (!exported) return;
    const timer = window.setTimeout(() => setExported(null), EXPORT_NOTICE_MS);
    return () => clearTimeout(timer);
  }, [exported]);

  const duplicate = (theme: Theme) =>
    onEdit({
      id: newThemeId(),
      name: t("theme.copyName", { name: theme.name }).slice(0, NAME_LIMIT),
      scheme: theme.scheme,
      tokens: { ...theme.tokens },
    });
  // A blank theme starts from the neutral palette of the active mode, not from a copy.
  const create = () => {
    const names = new Set(themes.map((theme) => theme.name));
    let name = t("theme.newName");
    for (let index = 2; names.has(name); index++)
      name = t("theme.newNameNumbered", { index });
    const base = defaultThemeFor(active.scheme);
    onEdit({
      id: newThemeId(),
      name,
      scheme: base.scheme,
      tokens: { ...base.tokens },
    });
  };
  const copyExport = () =>
    navigator.clipboard
      .writeText(exportTheme(active))
      .then(() => setExported("copied"))
      .catch(() => setExported("failed"));
  const remove = () => {
    if (window.confirm(t("theme.confirmDelete", { name: active.name })))
      themeStore.remove(active.id);
  };

  return (
    <SettingSection
      id="theme"
      title={t("settings.appearance.theme")}
      description={t("theme.subtitle")}
    >
      <div className="theme-grid" role="group" aria-label={t("theme.list")}>
        {themes.map((theme) => (
          <button
            key={theme.id}
            className="theme-card"
            aria-pressed={theme.id === active.id}
            onClick={() => themeStore.select(theme.id)}
          >
            <ThemePreview theme={theme} />
            <span className="theme-card-label">
              {theme.id === active.id && <Check size={13} />}
              <strong>{theme.name}</strong>
              <span className="badge">
                {t(
                  theme.builtIn
                    ? `theme.scheme.${theme.scheme}`
                    : "theme.custom",
                )}
              </span>
            </span>
          </button>
        ))}
        <button className="theme-card theme-card-new" onClick={create}>
          <span className="theme-preview">
            <Plus size={18} />
          </span>
          <span className="theme-card-label">
            <strong>{t("theme.new")}</strong>
          </span>
        </button>
      </div>
      <div className="theme-toolbar">
        <Button onClick={create}>
          <Plus />
          {t("theme.new")}
        </Button>
        {active.builtIn ? (
          <Button variant="outline" onClick={() => duplicate(active)}>
            <Palette />
            {t("theme.customize")}
          </Button>
        ) : (
          <Button variant="outline" onClick={() => onEdit(active)}>
            <Pencil />
            {t("theme.edit")}
          </Button>
        )}
        <Button variant="outline" onClick={() => duplicate(active)}>
          <Copy />
          {t("clipboard.duplicate")}
        </Button>
        <Button variant="outline" onClick={() => void copyExport()}>
          <Share />
          {t(
            exported === "copied"
              ? "theme.exportCopied"
              : exported === "failed"
                ? "theme.exportFailed"
                : "theme.export",
          )}
        </Button>
        <Button variant="outline" onClick={() => setImporting(true)}>
          <ImportIcon />
          {t("theme.import")}
        </Button>
        {!active.builtIn && (
          <Button variant="destructive" className="ml-auto" onClick={remove}>
            <Trash2 />
            {t("clipboard.delete")}
          </Button>
        )}
      </div>
      <ThemeImportDialog open={importing} onClose={() => setImporting(false)} />
    </SettingSection>
  );
}
