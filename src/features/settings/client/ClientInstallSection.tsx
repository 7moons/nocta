import {
  AlertCircle,
  CheckCircle2,
  Download,
  HardDrive,
  LoaderCircle,
  RefreshCw,
  X,
} from "lucide-react";
import type { AppConfig, ClientState } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { useT } from "@/i18n";
import { api } from "@/lib/ipc";
import { SettingRow, SettingSection } from "../SettingControls";
import type { ClientActions } from "./useClientActions";

const PHASES = ["preparing", "downloading", "extracting", "finishing"] as const;

/** The installed client, install progress and whether launches use it. */
export function ClientInstallSection({
  config,
  client,
  actions,
  onChange,
}: {
  config: AppConfig;
  client: ClientState;
  actions: ClientActions;
  onChange: (patch: Partial<AppConfig>) => void;
}) {
  const t = useT();
  const active = client.installed.find((item) => item.guid === client.active);
  const latest = client.latest;
  const outdated = !!latest && !!active && latest.guid !== active.guid;
  const latestInstalled =
    !!latest && client.installed.some((item) => item.guid === latest.guid);
  const progress = client.progress;
  const percent = progress?.totalMb
    ? Math.round((progress.doneMb / progress.totalMb) * 100)
    : 0;
  const phase = PHASES.find((item) => item === progress?.phase);
  const canLaunchWithIt = !!active || config.clientUpdateMode === "auto";

  return (
    <SettingSection
      id="client-install"
      title={t("settings.client.installation")}
      description={t("client.install.subtitle")}
    >
      <div className="client-status">
        <span className={`client-status-icon ${active ? "ready" : ""}`}>
          <HardDrive size={18} />
        </span>
        <div>
          <strong>
            {active
              ? t("client.version", { version: active.version ?? active.guid })
              : t("client.install.missing")}
          </strong>
          <small>
            {active ? (
              <>
                <code>{active.guid}</code> ·{" "}
                {t("client.sizeMb", { size: active.sizeMb })}
                {config.clientPinnedVersion === active.guid &&
                  ` · ${t("client.versions.pinned")}`}
              </>
            ) : (
              t("client.install.missingDetail")
            )}
          </small>
        </div>
        <span
          className={`client-latest ${outdated ? "outdated" : active && latest ? "current" : ""}`}
        >
          {!latest ? (
            t(
              actions.checking
                ? "client.latest.checking"
                : "client.latest.unknown",
            )
          ) : outdated ? (
            <>
              <AlertCircle size={13} />
              {t("client.latest.available", { version: latest.version })}
            </>
          ) : active ? (
            <>
              <CheckCircle2 size={13} />
              {t("client.latest.current")}
            </>
          ) : (
            t("client.latest.version", { version: latest.version })
          )}
        </span>
      </div>

      {progress && (
        <div className="client-progress" role="status">
          <div className="client-progress-head">
            <span>
              {phase ? t(`client.phase.${phase}`) : progress.phase}{" "}
              {progress.guid}
            </span>
            <span className="tabular">
              {t("client.install.progress", {
                done: progress.doneMb,
                total: progress.totalMb,
                packages: progress.packagesDone,
                packagesTotal: progress.packagesTotal,
              })}
            </span>
          </div>
          <div className="client-progress-bar">
            <span style={{ width: `${percent}%` }} />
          </div>
        </div>
      )}

      <div className="client-actions">
        {progress ? (
          <Button
            variant="outline"
            onClick={() => actions.call(api.clientCancelInstall)}
          >
            <X />
            {t("client.install.cancel")}
          </Button>
        ) : (
          <Button
            disabled={latestInstalled && !outdated}
            onClick={() => actions.call(() => api.clientInstall())}
          >
            <Download />
            {!active
              ? t("client.install.install")
              : outdated && !latestInstalled
                ? t("client.install.update", { version: latest?.version ?? "" })
                : t("client.install.reinstall")}
          </Button>
        )}
        <Button
          variant="outline"
          disabled={actions.checking || !!progress}
          onClick={actions.check}
        >
          {actions.checking ? <LoaderCircle className="spin" /> : <RefreshCw />}
          {t("client.install.check")}
        </Button>
      </div>
      {(actions.error || client.lastError) && (
        <div className="error-banner client-error" role="alert">
          <AlertCircle size={14} />
          <span>{actions.error || client.lastError}</span>
        </div>
      )}

      <SettingRow
        title={t("client.install.useIt")}
        description={t(
          canLaunchWithIt
            ? "client.install.useItDetail"
            : "client.install.useItFirst",
        )}
      >
        <Switch
          label={t("client.install.useIt")}
          checked={config.managedClient}
          disabled={!canLaunchWithIt}
          onCheckedChange={(value) => onChange({ managedClient: value })}
        />
      </SettingRow>
    </SettingSection>
  );
}
