import type {
  AppConfig,
  ClientGraphicsApi,
  ClientState,
  ClientUpdateMode,
} from "@/generated/contracts";
import { Switch } from "@/components/ui/switch";
import { useT } from "@/i18n";
import { ClientInstallSection } from "./client/ClientInstallSection";
import { ClientVersionsSection } from "./client/ClientVersionsSection";
import { useClientActions } from "./client/useClientActions";
import { FastFlagsSection } from "./client/FastFlagsSection";
import { NumberSetting, SettingRow, SettingSection } from "./SettingControls";

const GRAPHICS_APIS: ClientGraphicsApi[] = [
  "auto",
  "d3d11",
  "vulkan",
  "opengl",
];
const UPDATE_MODES: ClientUpdateMode[] = ["auto", "ask", "manual"];

/** Nocta's own Roblox install: status, performance, updates and installed versions. */
export function ClientSettings({
  config,
  client,
  onChange,
}: {
  config: AppConfig;
  client: ClientState;
  onChange: (patch: Partial<AppConfig>) => void;
}) {
  const t = useT();
  const actions = useClientActions(config, client);

  return (
    <>
      <ClientInstallSection
        config={config}
        client={client}
        actions={actions}
        onChange={onChange}
      />
      <SettingSection
        id="client-performance"
        title={t("settings.client.performance")}
        description={t("client.performance.subtitle")}
      >
        <SettingRow
          title={t("client.performance.lowGraphics")}
          description={t("client.performance.lowGraphicsDetail")}
        >
          <Switch
            label={t("client.performance.lowGraphics")}
            checked={config.clientLowGraphics}
            onCheckedChange={(value) => onChange({ clientLowGraphics: value })}
          />
        </SettingRow>
        <SettingRow
          title={t("client.performance.api")}
          description={t("client.performance.apiDetail")}
        >
          <select
            aria-label={t("client.performance.api")}
            value={config.clientGraphicsApi}
            onChange={(event) =>
              onChange({
                clientGraphicsApi: event.target.value as ClientGraphicsApi,
              })
            }
          >
            {GRAPHICS_APIS.map((api) => (
              <option key={api} value={api}>
                {t(`client.performance.apis.${api}`)}
              </option>
            ))}
          </select>
        </SettingRow>
        <p className="client-note">{t("client.performance.fpsNote")}</p>
      </SettingSection>

      <SettingSection
        id="client-updates"
        title={t("settings.client.updates")}
        description={t("client.updates.subtitle")}
      >
        <SettingRow
          title={t("client.updates.mode")}
          description={t(
            `client.updates.modeDetail.${config.clientUpdateMode}`,
          )}
        >
          <select
            aria-label={t("client.updates.mode")}
            value={config.clientUpdateMode}
            onChange={(event) =>
              onChange({
                clientUpdateMode: event.target.value as ClientUpdateMode,
              })
            }
          >
            {UPDATE_MODES.map((mode) => (
              <option key={mode} value={mode}>
                {t(`client.updates.modes.${mode}`)}
              </option>
            ))}
          </select>
        </SettingRow>
        <SettingRow
          title={t("client.updates.keep")}
          description={t("client.updates.keepDetail")}
        >
          <NumberSetting
            label={t("client.updates.keep")}
            value={config.clientKeepVersions}
            min={0}
            max={5}
            onCommit={(value) => onChange({ clientKeepVersions: value })}
          />
        </SettingRow>
      </SettingSection>

      <FastFlagsSection
        flags={config.fastFlags}
        onChange={(fastFlags) => onChange({ fastFlags })}
      />

      <ClientVersionsSection
        config={config}
        client={client}
        actions={actions}
        onChange={onChange}
      />
    </>
  );
}
