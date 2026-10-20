import type {
  AppConfig,
  ArrangeMonitors,
  ClientProfile,
  ProcessPriority,
} from "@/generated/contracts";
import { MemoryStick } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { useT, type MessageKey } from "@/i18n";
import { NumberSetting, SettingRow, SettingSection } from "../SettingControls";

type SwitchKey =
  | "nameWindows"
  | "autoArrange"
  | "closeHung"
  | "closeDisconnected"
  | "keepAwake"
  | "freeWindowResize";

const PRIORITIES: ProcessPriority[] = ["normal", "below_normal", "idle"];

/**
 * Other accounts' profile for running as many clients as memory allows: lower CPU priority and
 * efficiency mode, memory taken from them first, RAM capped at 400 MB and unused memory given
 * back every minute. Measured in a game: 939 MB of RAM without a cap, 350 MB with one, for 0.3%
 * more CPU. Main accounts are left as they are.
 */
const MANY_INSTANCES: Partial<ClientProfile> = {
  priority: "below_normal",
  ecoMode: true,
  lowMemoryPriority: true,
  trimEverySecs: 60,
  ramCapMb: 400,
};

/** Looking after running clients, and how much of the computer each may use. */
export function ClientCareSettings({
  config,
  onChange,
}: {
  config: AppConfig;
  onChange: (patch: Partial<AppConfig>) => void;
}) {
  const t = useT();
  const toggle = (key: SwitchKey) => (
    <SettingRow
      key={key}
      title={t(`settings.clients.${key}` as MessageKey)}
      description={t(`settings.clients.${key}Detail` as MessageKey)}
    >
      <Switch
        label={t(`settings.clients.${key}` as MessageKey)}
        checked={config[key]}
        onCheckedChange={(value) => onChange({ [key]: value })}
      />
    </SettingRow>
  );
  return (
    <>
      <SettingSection
        id="clients"
        title={t("settings.clients.title")}
        description={t("settings.clients.subtitle")}
      >
        <SettingRow
          title={t("launch.options.pacing")}
          description={t("settings.clients.pacingDetail")}
        >
          <Switch
            label={t("launch.options.pacing")}
            checked={config.launchPacing === "in_game"}
            onCheckedChange={(value) =>
              onChange({ launchPacing: value ? "in_game" : "fixed" })
            }
          />
        </SettingRow>
        <SettingRow
          title={t("settings.clients.timeout")}
          description={t("settings.clients.timeoutDetail")}
        >
          <NumberSetting
            label={t("settings.clients.timeout")}
            value={config.inGameTimeoutSecs}
            min={10}
            max={600}
            unit={t("common.unit.seconds")}
            onCommit={(value) => onChange({ inGameTimeoutSecs: value })}
          />
        </SettingRow>
        {toggle("nameWindows")}
        <SettingRow
          title={t("settings.clients.monitors")}
          description={t("settings.clients.monitorsDetail")}
        >
          <select
            aria-label={t("settings.clients.monitors")}
            value={config.arrangeMonitors}
            onChange={(event) =>
              onChange({
                arrangeMonitors: event.target.value as ArrangeMonitors,
              })
            }
          >
            <option value="all">{t("settings.clients.monitorsAll")}</option>
            <option value="primary">
              {t("settings.clients.monitorsPrimary")}
            </option>
          </select>
        </SettingRow>
        {toggle("autoArrange")}
        {toggle("closeHung")}
        {toggle("closeDisconnected")}
        <SettingRow
          title={t("settings.clients.restartAlts")}
          description={t("settings.clients.restartAltsDetail")}
        >
          <NumberSetting
            label={t("settings.clients.restartAlts")}
            value={config.restartAltsMinutes}
            max={1440}
            unit={t("common.unit.minutes")}
            onCommit={(value) => onChange({ restartAltsMinutes: value })}
          />
        </SettingRow>
        {toggle("keepAwake")}
        {toggle("freeWindowResize")}
      </SettingSection>
      <SettingSection
        id="profiles"
        title={t("settings.profiles.title")}
        description={t("settings.profiles.subtitle")}
      >
        <div className="client-profiles-actions">
          <Button
            variant="outline"
            size="sm"
            onClick={() =>
              onChange({
                altProfile: { ...config.altProfile, ...MANY_INSTANCES },
                clientLowGraphics: true,
              })
            }
          >
            <MemoryStick />
            {t("settings.profiles.manyInstances")}
          </Button>
          <small className="field-hint">
            {t("settings.profiles.manyInstancesDetail")}
          </small>
        </div>
        <div className="client-profiles">
          {(["mainProfile", "altProfile"] as const).map((key) => (
            <ProfileEditor
              key={key}
              title={t(
                key === "mainProfile"
                  ? "settings.profiles.main"
                  : "settings.profiles.alt",
              )}
              profile={config[key]}
              onChange={(profile) => onChange({ [key]: profile })}
            />
          ))}
        </div>
      </SettingSection>
    </>
  );
}

function ProfileEditor({
  title,
  profile,
  onChange,
}: {
  title: string;
  profile: ClientProfile;
  onChange: (profile: ClientProfile) => void;
}) {
  const t = useT();
  const set = (patch: Partial<ClientProfile>) =>
    onChange({ ...profile, ...patch });
  const label = (key: MessageKey) => `${title}: ${t(key)}`;
  return (
    <fieldset className="client-profile">
      <legend>{title}</legend>
      <label className="field-label">
        {t("settings.profiles.priority")}
        <select
          aria-label={label("settings.profiles.priority")}
          value={profile.priority}
          onChange={(event) =>
            set({ priority: event.target.value as ProcessPriority })
          }
        >
          {PRIORITIES.map((priority) => (
            <option key={priority} value={priority}>
              {t(`settings.profiles.priorities.${priority}`)}
            </option>
          ))}
        </select>
      </label>
      <label className="checkbox-label">
        <input
          type="checkbox"
          aria-label={label("settings.profiles.eco")}
          checked={profile.ecoMode}
          onChange={(event) => set({ ecoMode: event.target.checked })}
        />
        {t("settings.profiles.eco")}
      </label>
      <label className="checkbox-label">
        <input
          type="checkbox"
          aria-label={label("settings.profiles.lowMemory")}
          checked={profile.lowMemoryPriority}
          onChange={(event) => set({ lowMemoryPriority: event.target.checked })}
        />
        {t("settings.profiles.lowMemory")}
      </label>
      <label className="field-label">
        {t("settings.profiles.ramCap")}
        <NumberSetting
          label={label("settings.profiles.ramCap")}
          value={profile.ramCapMb}
          max={16384}
          unit="MB"
          onCommit={(value) =>
            set({ ramCapMb: value === 0 ? 0 : Math.max(100, value) })
          }
        />
        <small className="field-hint">
          {t("settings.profiles.ramCapHint")}
        </small>
      </label>
      <label className="field-label">
        {t("settings.profiles.trim")}
        <NumberSetting
          label={label("settings.profiles.trim")}
          value={profile.trimEverySecs}
          max={3600}
          unit={t("common.unit.seconds")}
          onCommit={(value) =>
            set({ trimEverySecs: value === 0 ? 0 : Math.max(30, value) })
          }
        />
      </label>
      <label className="field-label">
        {t("settings.profiles.cpu")}
        <NumberSetting
          label={label("settings.profiles.cpu")}
          value={profile.cpuLimitPercent}
          max={100}
          unit="%"
          onCommit={(value) => set({ cpuLimitPercent: value })}
        />
      </label>
      <div className="field-label">
        {t("settings.profiles.window")}
        <span className="client-profile-size">
          <NumberSetting
            label={label("settings.profiles.width")}
            value={profile.windowWidth}
            max={7680}
            onCommit={(value) =>
              set({
                windowWidth: value,
                windowHeight: value === 0 ? 0 : profile.windowHeight || 600,
              })
            }
          />
          ×
          <NumberSetting
            label={label("settings.profiles.height")}
            value={profile.windowHeight}
            max={4320}
            onCommit={(value) =>
              set({
                windowHeight: value,
                windowWidth: value === 0 ? 0 : profile.windowWidth || 800,
              })
            }
          />
        </span>
        <small className="field-hint">
          {t("settings.profiles.windowHint")}
        </small>
      </div>
      <small className="field-hint">{t("settings.profiles.hint")}</small>
    </fieldset>
  );
}
