import { useState } from "react";
import { Download, LoaderCircle, X } from "lucide-react";
import type { AppConfig, ClientState } from "@/generated/contracts";
import { api } from "@/lib/ipc";
import { errorMessage } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";

// Shows a running install of Nocta's Roblox client, or an available update when the
// user chose to update by hand ("Ask first" or "Manual").
export function ClientUpdateBanner({
  config,
  client,
  onError,
}: {
  config: AppConfig;
  client: ClientState;
  onError: (message: string) => void;
}) {
  const t = useT();
  const [dismissed, setDismissed] = useState<string | null>(null);
  const progress = client.progress;
  if (progress) {
    const percent = progress.totalMb
      ? Math.round((progress.doneMb / progress.totalMb) * 100)
      : 0;
    return (
      <div className="client-banner" role="status">
        <LoaderCircle className="spin" size={14} />
        <span>
          {t("client.banner.installing", {
            done: progress.doneMb,
            total: progress.totalMb,
            percent,
          })}
        </span>
        <span className="client-banner-bar">
          <span style={{ width: `${percent}%` }} />
        </span>
      </div>
    );
  }
  const latest = client.latest;
  const outdated =
    config.managedClient &&
    config.clientUpdateMode !== "auto" &&
    !config.clientPinnedVersion &&
    !!latest &&
    !!client.active &&
    latest.guid !== client.active &&
    !client.installed.some((item) => item.guid === latest.guid);
  if (!outdated || dismissed === latest.guid) return null;
  return (
    <div className="client-banner" role="status">
      <Download size={14} />
      <span>{t("client.banner.available", { version: latest.version })}</span>
      <Button
        size="sm"
        onClick={() =>
          void api
            .clientInstall()
            .catch((error) => onError(errorMessage(error)))
        }
      >
        {t("client.banner.update")}
      </Button>
      <Button
        variant="ghost"
        size="icon"
        aria-label={t("client.banner.dismiss")}
        onClick={() => setDismissed(latest.guid)}
      >
        <X />
      </Button>
    </div>
  );
}
