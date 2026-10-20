import { useState } from "react";
import type { AppConfig, RuntimeInfo } from "@/generated/contracts";
import { Switch } from "@/components/ui/switch";
import {
  QUICK_ACTIONS,
  useQuickActions,
} from "@/features/accounts/quickActions";
import { useT } from "@/i18n";
import { api } from "@/lib/ipc";
import type { Theme } from "@/theme/theme";
import { NumberSetting, SettingRow, SettingSection } from "../SettingControls";
import { ThemeEditor } from "./ThemeEditor";
import { ThemeSettings } from "./ThemeSettings";

/** Theme, display size and the quick actions shown on account rows. */
export function AppearanceSettings({
  config,
  runtime,
  onChange,
}: {
  config: AppConfig;
  runtime: RuntimeInfo;
  onChange: (patch: Partial<AppConfig>) => void;
}) {
  const t = useT();
  const { enabled, toggle } = useQuickActions();
  const [editing, setEditing] = useState<Theme | null>(null);

  if (editing)
    return <ThemeEditor theme={editing} onClose={() => setEditing(null)} />;

  return (
    <>
      <ThemeSettings onEdit={setEditing} />
      <SettingSection
        id="display"
        title={t("settings.appearance.display")}
        description={t("settings.appearance.displayDetail")}
      >
        <SettingRow
          title={t("settings.appearance.scale")}
          description={t("settings.appearance.scaleDetail")}
        >
          <NumberSetting
            label={t("settings.appearance.scale")}
            value={config.interfaceScale}
            min={75}
            max={150}
            onCommit={(value) => onChange({ interfaceScale: value })}
            unit="%"
          />
        </SettingRow>
        <SettingRow
          title={t("settings.appearance.statusBar")}
          description={t("settings.appearance.statusBarDetail")}
        >
          <Switch
            label={t("settings.appearance.statusBar")}
            checked={config.showStatusBar}
            onCheckedChange={(value) => onChange({ showStatusBar: value })}
          />
        </SettingRow>
        <SettingRow
          title={t("settings.appearance.compact")}
          description={t("settings.appearance.compactDetail")}
        >
          <Switch
            label={t("settings.appearance.compact")}
            checked={config.compactList}
            onCheckedChange={(value) => onChange({ compactList: value })}
          />
        </SettingRow>
        <SettingRow
          title={t("settings.appearance.streamer")}
          description={t("settings.appearance.streamerDetail")}
        >
          <Switch
            label={t("settings.appearance.streamer")}
            checked={config.streamerMode}
            onCheckedChange={(value) => onChange({ streamerMode: value })}
          />
        </SettingRow>
        {runtime.platform === "windows" && (
          <SettingRow
            title={t("settings.appearance.safeDisplay")}
            description={t("settings.appearance.safeDisplayDetail")}
          >
            <Switch
              label={t("settings.appearance.safeDisplay")}
              // On means GPU acceleration; off (the default) draws without the GPU.
              checked={!runtime.safeDisplay}
              onCheckedChange={(value) => {
                if (window.confirm(t("settings.appearance.safeDisplayRestart")))
                  void api.setSafeDisplay(!value);
              }}
            />
          </SettingRow>
        )}
      </SettingSection>
      <SettingSection
        id="quick-actions"
        title={t("settings.appearance.quickActions")}
        description={t("settings.appearance.quickActionsDetail")}
      >
        {QUICK_ACTIONS.map((action) => {
          const label = t(`quickActions.${action.id}`);
          return (
            <SettingRow
              key={action.id}
              title={label}
              description={t(
                `settings.appearance.quickActionHint.${action.id}`,
              )}
            >
              <span className="quick-action-setting">
                <action.icon size={14} />
                <Switch
                  label={t("settings.appearance.showButton", { name: label })}
                  checked={enabled.includes(action.id)}
                  onCheckedChange={(value) => toggle(action.id, value)}
                />
              </span>
            </SettingRow>
          );
        })}
      </SettingSection>
    </>
  );
}
