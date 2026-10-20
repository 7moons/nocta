import type { AppConfig } from "@/generated/contracts";
import { useT } from "@/i18n";
import { NumberSetting, SettingRow, SettingSection } from "../SettingControls";

type NumberKey = "browserTimeoutSecs" | "checkerDelaySecs" | "precheckRetries";

// Limits match the backend's config validation.
const TIMING: { key: NumberKey; min: number; max: number; seconds: boolean }[] =
  [
    { key: "browserTimeoutSecs", min: 10, max: 3600, seconds: true },
    { key: "checkerDelaySecs", min: 0, max: 3600, seconds: true },
    { key: "precheckRetries", min: 0, max: 10, seconds: false },
  ];

/** How session checks run: how many at once, their timeouts, delays and retries. */
export function SessionChecksSettings({
  config,
  onChange,
}: {
  config: AppConfig;
  onChange: (patch: Partial<AppConfig>) => void;
}) {
  const t = useT();
  return (
    <>
      <SettingSection
        id="concurrency"
        title={t("settings.checks.concurrency")}
        description={t("settings.checks.concurrencyDetail")}
      >
        <SettingRow
          title={t("settings.checks.checkers")}
          description={t("settings.checks.checkersDetail")}
        >
          <NumberSetting
            label={t("settings.checks.checkers")}
            value={config.checkerConcurrency}
            min={1}
            max={100}
            onCommit={(value) => onChange({ checkerConcurrency: value })}
          />
        </SettingRow>
      </SettingSection>
      <SettingSection
        id="timing"
        title={t("settings.checks.timing")}
        description={t("settings.checks.timingDetail")}
      >
        {TIMING.map(({ key, min, max, seconds }) => (
          <SettingRow
            key={key}
            title={t(`settings.checks.${key}`)}
            description={t(`settings.checks.${key}Detail`)}
          >
            <NumberSetting
              label={t(`settings.checks.${key}`)}
              value={config[key]}
              min={min}
              max={max}
              unit={seconds ? t("common.unit.seconds") : undefined}
              onCommit={(value) => onChange({ [key]: value })}
            />
          </SettingRow>
        ))}
      </SettingSection>
    </>
  );
}
