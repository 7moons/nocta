import type { AppConfig } from "@/generated/contracts";
import { Switch } from "@/components/ui/switch";
import { HotkeyInput } from "@/components/HotkeyInput";
import { ClipboardFormatManager } from "@/features/accounts/ClipboardFormatManager";
import {
  LANGUAGES,
  languageStore,
  useLanguage,
  useT,
  type LanguageId,
} from "@/i18n";
import { CLOCKS, clockStore, useClock, type Clock } from "@/i18n/clock";
import {
  NumberSetting,
  SettingRow,
  SettingSection,
  TextSetting,
} from "../SettingControls";

export function GeneralSettings({
  config,
  onChange,
}: {
  config: AppConfig;
  onChange: (patch: Partial<AppConfig>) => void;
}) {
  const t = useT();
  const language = useLanguage();
  const clock = useClock();
  return (
    <>
      <SettingSection
        id="general"
        title={t("settings.general.title")}
        description={t("settings.general.subtitle")}
      >
        <SettingRow
          title={t("settings.general.language")}
          description={t("settings.general.languageDetail")}
        >
          <select
            aria-label={t("settings.general.language")}
            value={language}
            onChange={(event) =>
              languageStore.set(event.target.value as LanguageId)
            }
          >
            {LANGUAGES.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </select>
        </SettingRow>
        <SettingRow
          title={t("settings.general.clock")}
          description={t("settings.general.clockDetail")}
        >
          <select
            aria-label={t("settings.general.clock")}
            value={clock}
            onChange={(event) => clockStore.set(event.target.value as Clock)}
          >
            {CLOCKS.map((item) => (
              <option key={item} value={item}>
                {t(`settings.general.clockOptions.${item}`)}
              </option>
            ))}
          </select>
        </SettingRow>
        <SettingRow
          title={t("settings.general.alwaysOnTop")}
          description={t("settings.general.alwaysOnTopDetail")}
        >
          <Switch
            label={t("settings.general.alwaysOnTop")}
            checked={config.alwaysOnTop}
            onCheckedChange={(value) => onChange({ alwaysOnTop: value })}
          />
        </SettingRow>
        <SettingRow
          title={t("settings.general.bulkInterval")}
          description={t("settings.general.bulkIntervalDetail")}
        >
          <NumberSetting
            label={t("settings.general.bulkInterval")}
            value={config.bulkActionDelaySecs}
            onCommit={(value) => onChange({ bulkActionDelaySecs: value })}
            unit={t("common.unit.seconds")}
          />
        </SettingRow>
        <SettingRow
          title={t("settings.general.separators")}
          description={t("settings.general.separatorsDetail")}
        >
          <TextSetting
            label={t("settings.general.separators")}
            value={config.searchSeparators}
            onCommit={(value) => onChange({ searchSeparators: value })}
          />
        </SettingRow>
      </SettingSection>
      <SettingSection
        id="system"
        title={t("settings.system.title")}
        description={t("settings.system.subtitle")}
      >
        <SettingRow
          title={t("settings.system.tray")}
          description={t("settings.system.trayDetail")}
        >
          <Switch
            label={t("settings.system.tray")}
            checked={config.closeToTray}
            onCheckedChange={(value) => onChange({ closeToTray: value })}
          />
        </SettingRow>
        <SettingRow
          title={t("settings.system.autostart")}
          description={t("settings.system.autostartDetail")}
        >
          <Switch
            label={t("settings.system.autostart")}
            checked={config.startWithWindows}
            onCheckedChange={(value) => onChange({ startWithWindows: value })}
          />
        </SettingRow>
        <SettingRow
          title={t("settings.system.minimized")}
          description={t("settings.system.minimizedDetail")}
        >
          <Switch
            label={t("settings.system.minimized")}
            checked={config.startMinimized}
            disabled={!config.startWithWindows}
            onCheckedChange={(value) => onChange({ startMinimized: value })}
          />
        </SettingRow>
      </SettingSection>
      <SettingSection
        id="hotkeys"
        title={t("hotkeys.title")}
        description={t("hotkeys.subtitle")}
      >
        {(["show", "closeRoblox", "toggleAfk"] as const).map((key) => (
          <SettingRow
            key={key}
            title={t(`hotkeys.${key}`)}
            description={t(`hotkeys.${key}Detail`)}
          >
            <HotkeyInput
              label={t(`hotkeys.${key}`)}
              value={config.hotkeys[key]}
              onChange={(value) =>
                onChange({ hotkeys: { ...config.hotkeys, [key]: value } })
              }
            />
          </SettingRow>
        ))}
      </SettingSection>
      <SettingSection
        id="clipboard"
        title={t("clipboard.title")}
        description={t("settings.clipboard.subtitle")}
      >
        <div className="settings-format-manager">
          <ClipboardFormatManager />
        </div>
      </SettingSection>
    </>
  );
}
