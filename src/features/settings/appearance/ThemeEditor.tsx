import { useEffect, useState } from "react";
import { RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { defaultThemeFor } from "@/theme/presets";
import { themeStore, useThemes } from "@/theme/store";
import {
  isColor,
  splitColor,
  type ColorScheme,
  type Theme,
} from "@/theme/theme";
import {
  THEME_TOKENS,
  TOKEN_GROUPS,
  tokenKey,
  type ThemeTokenName,
} from "@/theme/tokens";
import { SettingSection } from "../SettingControls";

const NAME_LIMIT = 40;

/**
 * Edits a custom theme. Every change previews live across the whole app; nothing is stored
 * until Save, and leaving the editor restores the active theme.
 */
export function ThemeEditor({
  theme,
  onClose,
}: {
  theme: Theme;
  onClose: () => void;
}) {
  const t = useT();
  const { themes } = useThemes();
  const [draft, setDraft] = useState(theme);
  useEffect(() => themeStore.preview(draft), [draft]);
  useEffect(() => () => themeStore.preview(null), []);
  const setToken = (name: ThemeTokenName, value: string) =>
    setDraft((current) => ({
      ...current,
      tokens: { ...current.tokens, [name]: value },
    }));
  // While the colors are still an untouched base palette, switching the mode
  // swaps in the other base palette; edited colors are never overwritten.
  const setScheme = (scheme: ColorScheme) =>
    setDraft((current) => {
      const untouched =
        JSON.stringify(current.tokens) ===
        JSON.stringify(defaultThemeFor(current.scheme).tokens);
      return {
        ...current,
        scheme,
        tokens: untouched
          ? { ...defaultThemeFor(scheme).tokens }
          : current.tokens,
      };
    });
  const loadColors = (id: string) => {
    const source = themes.find((item) => item.id === id);
    if (source)
      setDraft((current) => ({
        ...current,
        scheme: source.scheme,
        tokens: { ...source.tokens },
      }));
  };
  const save = () => {
    themeStore.save({
      ...draft,
      name: draft.name.trim() || t("theme.untitled"),
      builtIn: false,
    });
    onClose();
  };
  // A fresh copy can be saved as-is; an existing theme only once something changed.
  const isNew = !themes.some((item) => item.id === theme.id);
  const changed = isNew || JSON.stringify(draft) !== JSON.stringify(theme);
  return (
    <>
      <div className="theme-editor-bar">
        <label className="field-label">
          {t("theme.editor.name")}
          <input
            autoFocus
            maxLength={NAME_LIMIT}
            value={draft.name}
            onChange={(event) =>
              setDraft({ ...draft, name: event.target.value })
            }
          />
        </label>
        <label className="field-label">
          {t("theme.editor.mode")}
          <select
            value={draft.scheme}
            onChange={(event) => setScheme(event.target.value as ColorScheme)}
          >
            <option value="dark">{t("theme.scheme.dark")}</option>
            <option value="light">{t("theme.scheme.light")}</option>
          </select>
        </label>
        <label className="field-label">
          {t("theme.editor.loadFrom")}
          <select value="" onChange={(event) => loadColors(event.target.value)}>
            <option value="" disabled>
              {t("theme.editor.choose")}
            </option>
            {themes.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </select>
        </label>
        <div className="dialog-actions">
          <Button variant="outline" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button onClick={save} disabled={!draft.name.trim() || !changed}>
            {t("theme.editor.save")}
          </Button>
        </div>
        <p className="theme-editor-note">{t("theme.editor.note")}</p>
      </div>
      {TOKEN_GROUPS.map((group) => (
        <SettingSection
          key={group}
          id={`theme-${group}`}
          title={t(`theme.group.${group}.title`)}
          description={t(`theme.group.${group}.description`)}
        >
          <div className="token-grid">
            {THEME_TOKENS.filter((token) => token.group === group).map(
              (token) => (
                <TokenRow
                  key={token.name}
                  name={token.name}
                  value={draft.tokens[token.name]}
                  original={theme.tokens[token.name]}
                  onChange={(value) => setToken(token.name, value)}
                />
              ),
            )}
          </div>
        </SettingSection>
      ))}
    </>
  );
}

function TokenRow({
  name,
  value,
  original,
  onChange,
}: {
  name: ThemeTokenName;
  value: string;
  original: string;
  onChange: (value: string) => void;
}) {
  const t = useT();
  const label = t(`theme.token.${tokenKey(name)}.label`);
  const hint = t(`theme.token.${tokenKey(name)}.hint`);
  // The text field may hold an unfinished value while focused; only valid colors reach
  // the theme, and leaving the field shows the theme's value again.
  const [text, setText] = useState<string | null>(null);
  const { hex, alpha } = splitColor(value);
  return (
    <div className="token-row">
      <label className="token-swatch" title={t("theme.editor.pick")}>
        <span style={{ background: value }} />
        <input
          type="color"
          aria-label={t("theme.editor.picker", { name: label })}
          value={hex}
          onChange={(event) => onChange(event.target.value + alpha)}
        />
      </label>
      <div>
        <h4>
          {label} <code>--{name}</code>
        </h4>
        <p title={hint}>{hint}</p>
      </div>
      <div className="token-value">
        <input
          aria-label={t("theme.editor.value", { name: label })}
          spellCheck={false}
          value={text ?? value}
          aria-invalid={text !== null && !isColor(text)}
          onChange={(event) => {
            setText(event.target.value);
            if (isColor(event.target.value))
              onChange(event.target.value.trim());
          }}
          onBlur={() => setText(null)}
        />
        <Button
          variant="ghost"
          size="icon"
          className={value !== original ? "visible" : ""}
          aria-label={t("theme.editor.reset", { name: label })}
          title={t("theme.editor.resetTitle")}
          onClick={() => onChange(original)}
        >
          <RotateCcw />
        </Button>
      </div>
    </div>
  );
}
