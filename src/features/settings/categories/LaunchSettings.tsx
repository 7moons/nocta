import { useState } from "react";
import type {
  AntiAfkAction,
  AppConfig,
  LauncherState,
} from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { useT, type MessageKey } from "@/i18n";
import { api } from "@/lib/ipc";
import { errorMessage } from "@/lib/utils";
import {
  NumberSetting,
  SettingRow,
  SettingSection,
  TextSetting,
} from "../SettingControls";

type SwitchKey =
  | "launchPrecheck"
  | "multiInstance"
  | "autoFixPrivateInvitePrivacy"
  | "afkMode"
  | "afkMinimize"
  | "autoRejoin"
  | "trimMemory"
  | "antiAfk"
  | "antiAfkRandom";

export function LaunchSettings({
  config,
  launcher,
  onChange,
}: {
  config: AppConfig;
  launcher: LauncherState;
  onChange: (patch: Partial<AppConfig>) => void;
}) {
  const t = useT();
  const toggle = (key: SwitchKey, title: MessageKey, detail: MessageKey) => (
    <SettingRow key={key} title={t(title)} description={t(detail)}>
      <Switch
        label={t(title)}
        checked={config[key]}
        onCheckedChange={(value) => onChange({ [key]: value })}
      />
    </SettingRow>
  );
  return (
    <>
      <SettingSection
        id="launch"
        title={t("settings.launch.title")}
        description={t("settings.launch.subtitle")}
      >
        {toggle(
          "launchPrecheck",
          "settings.launch.precheck",
          "settings.launch.precheckDetail",
        )}
        <SettingRow
          title={t("settings.launch.delay")}
          description={t("launch.options.delayDetail")}
        >
          <NumberSetting
            label={t("settings.launch.delay")}
            value={config.preLaunchDelaySecs}
            onCommit={(value) => onChange({ preLaunchDelaySecs: value })}
            unit={t("common.unit.seconds")}
          />
        </SettingRow>
        {launcher.multiInstanceSupported &&
          toggle(
            "multiInstance",
            "launch.options.multi",
            "launch.options.multiDetail",
          )}
        {toggle(
          "autoFixPrivateInvitePrivacy",
          "settings.launch.privacy",
          "settings.launch.privacyDetail",
        )}
      </SettingSection>
      <SettingSection
        id="performance"
        title={t("settings.performance.title")}
        description={t("settings.performance.subtitle")}
      >
        <SettingRow
          title={t("settings.performance.fpsCap")}
          description={t("settings.performance.fpsCapDetail")}
        >
          <NumberSetting
            label={t("settings.performance.fpsCap")}
            value={config.fpsCap}
            max={1000}
            unit={t("settings.performance.fps")}
            onCommit={(value) => onChange({ fpsCap: value })}
          />
        </SettingRow>
        {toggle(
          "afkMode",
          "launch.options.afk",
          "settings.performance.afkDetail",
        )}
        <SettingRow
          title={t("settings.performance.afkFps")}
          description={t("settings.performance.afkFpsDetail")}
        >
          <NumberSetting
            label={t("settings.performance.afkFps")}
            value={config.afkFpsCap}
            min={5}
            max={240}
            unit={t("settings.performance.fps")}
            onCommit={(value) => onChange({ afkFpsCap: value })}
          />
        </SettingRow>
        {toggle(
          "afkMinimize",
          "settings.performance.minimize",
          "settings.performance.minimizeDetail",
        )}
        {toggle(
          "trimMemory",
          "settings.performance.trim",
          "settings.performance.trimDetail",
        )}
        <SettingRow
          title={t("settings.performance.memoryLimit")}
          description={t("settings.performance.memoryLimitDetail")}
        >
          <NumberSetting
            label={t("settings.performance.memoryLimit")}
            value={config.memoryLimitMb}
            max={16384}
            unit="MB"
            onCommit={(value) => onChange({ memoryLimitMb: value })}
          />
        </SettingRow>
      </SettingSection>
      <SettingSection
        id="anti-afk"
        title={t("settings.antiAfk.title")}
        description={t("settings.antiAfk.subtitle")}
      >
        {toggle(
          "antiAfk",
          "settings.antiAfk.enabled",
          "settings.antiAfk.enabledDetail",
        )}
        <SettingRow
          title={t("settings.antiAfk.every")}
          description={t("settings.antiAfk.everyDetail")}
        >
          <NumberSetting
            label={t("settings.antiAfk.every")}
            value={config.antiAfkMinutes}
            min={1}
            max={19}
            unit={t("common.unit.minutes")}
            onCommit={(value) => onChange({ antiAfkMinutes: value })}
          />
        </SettingRow>
        {toggle(
          "antiAfkRandom",
          "settings.antiAfk.random",
          "settings.antiAfk.randomDetail",
        )}
        <SettingRow
          title={t("settings.antiAfk.action")}
          description={t("settings.antiAfk.actionDetail")}
        >
          <select
            aria-label={t("settings.antiAfk.action")}
            value={config.antiAfkAction}
            onChange={(event) =>
              onChange({ antiAfkAction: event.target.value as AntiAfkAction })
            }
          >
            <option value="turn">{t("settings.antiAfk.turn")}</option>
            <option value="jump">{t("settings.antiAfk.jump")}</option>
            <option value="click">{t("settings.antiAfk.click")}</option>
          </select>
        </SettingRow>
        {config.antiAfkAction === "click" && (
          <SettingRow
            title={t("settings.antiAfk.clickPoint")}
            description={t("settings.antiAfk.clickPointDetail")}
          >
            <span className="click-point-inputs">
              <NumberSetting
                label={t("accounts.details.afkX")}
                value={config.antiAfkClick.xPercent}
                max={100}
                unit="%"
                onCommit={(value) =>
                  onChange({
                    antiAfkClick: { ...config.antiAfkClick, xPercent: value },
                  })
                }
              />
              <NumberSetting
                label={t("accounts.details.afkY")}
                value={config.antiAfkClick.yPercent}
                max={100}
                unit="%"
                onCommit={(value) =>
                  onChange({
                    antiAfkClick: { ...config.antiAfkClick, yPercent: value },
                  })
                }
              />
            </span>
          </SettingRow>
        )}
      </SettingSection>
      <SettingSection
        id="recovery"
        title={t("settings.recovery.title")}
        description={t("settings.recovery.subtitle")}
      >
        {toggle(
          "autoRejoin",
          "launch.options.rejoin",
          "settings.recovery.rejoinDetail",
        )}
        <SettingRow
          title={t("settings.recovery.webhook")}
          description={t("settings.recovery.webhookDetail")}
        >
          <WebhookSetting
            value={config.discordWebhook}
            onCommit={(value) => onChange({ discordWebhook: value })}
          />
        </SettingRow>
      </SettingSection>
    </>
  );
}

/** The Discord webhook link with a button that sends a test message. */
function WebhookSetting({
  value,
  onCommit,
}: {
  value: string;
  onCommit: (value: string) => void;
}) {
  const t = useT();
  const [state, setState] = useState<{ busy: boolean; result: string }>({
    busy: false,
    result: "",
  });
  return (
    <div className="webhook-setting">
      <TextSetting
        label={t("settings.recovery.webhook")}
        placeholder="https://discord.com/api/webhooks/…"
        value={value}
        onCommit={onCommit}
      />
      <Button
        variant="outline"
        size="sm"
        disabled={!value || state.busy}
        onClick={async () => {
          setState({ busy: true, result: "" });
          try {
            await api.testWebhook(value);
            setState({ busy: false, result: t("settings.recovery.sent") });
          } catch (failure) {
            setState({ busy: false, result: errorMessage(failure) });
          }
        }}
      >
        {t("settings.recovery.test")}
      </Button>
      {state.result && (
        <small className="webhook-result" role="status">
          {state.result}
        </small>
      )}
    </div>
  );
}
