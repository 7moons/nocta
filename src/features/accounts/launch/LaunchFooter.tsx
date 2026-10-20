import type { ReactNode } from "react";
import { ChevronDown, Play, SlidersHorizontal } from "lucide-react";
import type { AppConfig, LauncherState } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { NumberSetting } from "@/features/settings/SettingControls";
import { useUiPreference } from "@/hooks/useUiPreference";
import { useT } from "@/i18n";

/** Launch options (folded away by default), a summary of the target and the launch button. */
export function LaunchFooter({
  config,
  launcher,
  selectedCount,
  ready,
  summary,
  onConfig,
  onLaunch,
}: {
  config: AppConfig;
  launcher: LauncherState;
  selectedCount: number;
  ready: boolean;
  summary: string;
  onConfig: (patch: Partial<AppConfig>) => void;
  onLaunch: () => void;
}) {
  const t = useT();
  const [open, setOpen] = useUiPreference("launchOptionsOpen", false);
  return (
    <footer className="launch-footer">
      {open && (
        <div
          className="launch-options"
          aria-label={t("launch.options.title")}
          role="group"
        >
          <OptionRow
            title={t("launch.options.between")}
            detail={t("launch.options.betweenDetail")}
          >
            <NumberSetting
              label={t("launch.options.between")}
              value={config.bulkActionDelaySecs}
              onCommit={(value) => onConfig({ bulkActionDelaySecs: value })}
              unit={t("common.unit.seconds")}
            />
          </OptionRow>
          <OptionRow
            title={t("launch.options.delay")}
            detail={t("launch.options.delayDetail")}
          >
            <NumberSetting
              label={t("launch.options.delay")}
              value={config.preLaunchDelaySecs}
              onCommit={(value) => onConfig({ preLaunchDelaySecs: value })}
              unit={t("common.unit.seconds")}
            />
          </OptionRow>
          <OptionRow
            title={t("launch.options.pacing")}
            detail={t("launch.options.pacingDetail")}
          >
            <Switch
              label={t("launch.options.pacing")}
              checked={config.launchPacing === "in_game"}
              onCheckedChange={(value) =>
                onConfig({ launchPacing: value ? "in_game" : "fixed" })
              }
            />
          </OptionRow>
          <OptionRow
            title={t("launch.options.precheck")}
            detail={t("launch.options.precheckDetail")}
          >
            <Switch
              label={t("launch.options.precheck")}
              checked={config.launchPrecheck}
              onCheckedChange={(value) => onConfig({ launchPrecheck: value })}
            />
          </OptionRow>
          <OptionRow
            title={t("launch.options.afk")}
            detail={t("launch.options.afkDetail", { fps: config.afkFpsCap })}
          >
            <Switch
              label={t("launch.options.afk")}
              checked={config.afkMode}
              onCheckedChange={(value) => onConfig({ afkMode: value })}
            />
          </OptionRow>
          <OptionRow
            title={t("launch.options.rejoin")}
            detail={t("launch.options.rejoinDetail")}
          >
            <Switch
              label={t("launch.options.rejoin")}
              checked={config.autoRejoin}
              onCheckedChange={(value) => onConfig({ autoRejoin: value })}
            />
          </OptionRow>
          {launcher.multiInstanceSupported && (
            <OptionRow
              title={t("launch.options.multi")}
              detail={t("launch.options.multiDetail")}
            >
              <Switch
                label={t("launch.options.multi")}
                checked={config.multiInstance}
                onCheckedChange={(value) => onConfig({ multiInstance: value })}
              />
            </OptionRow>
          )}
        </div>
      )}
      <div className="launch-footer-bar">
        <button
          className="launch-options-toggle"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
        >
          <SlidersHorizontal size={13} />
          {t("launch.options.toggle")}
          <ChevronDown size={13} className={open ? "flip" : undefined} />
        </button>
        <span className="launch-footer-summary" title={summary}>
          {summary}
        </span>
        <Button
          disabled={!ready || selectedCount === 0}
          title={selectedCount === 0 ? t("launch.selectFirst") : undefined}
          onClick={onLaunch}
        >
          <Play />
          {selectedCount === 0
            ? t("launch.selectToLaunch")
            : t("launch.launchCount", { count: selectedCount })}
        </Button>
      </div>
    </footer>
  );
}

function OptionRow({
  title,
  detail,
  children,
}: {
  title: string;
  detail: string;
  children: ReactNode;
}) {
  return (
    <div className="launch-option-row">
      <span>
        <strong>{title}</strong>
        <small>{detail}</small>
      </span>
      {children}
    </div>
  );
}
