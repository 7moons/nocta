import { useState } from "react";
import { Download, Pin, Trash2 } from "lucide-react";
import type { AppConfig, ClientState } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import { api } from "@/lib/ipc";
import { SettingRow, SettingSection } from "../SettingControls";
import type { ClientActions } from "./useClientActions";

const VERSION_PATTERN = /^version-[0-9a-f]{16}$/;

/** Installed versions: pin one, remove old ones, or install a specific version. */
export function ClientVersionsSection({
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
  const format = useFormat();
  const [specific, setSpecific] = useState("");
  const busy = !!client.progress;
  return (
    <SettingSection
      id="client-versions"
      title={t("settings.client.versions")}
      description={t("client.versions.subtitle")}
    >
      <div
        className="client-versions"
        role="list"
        aria-label={t("client.versions.list")}
      >
        {client.installed.length === 0 && (
          <p className="client-note">{t("client.versions.none")}</p>
        )}
        {client.installed.map((item) => {
          const isActive = item.guid === client.active;
          const pinned = item.guid === config.clientPinnedVersion;
          return (
            <div
              key={item.guid}
              role="listitem"
              className={`client-version ${isActive ? "active" : ""}`}
            >
              <div>
                <strong>
                  {item.version ?? t("client.versions.unknown")}
                  {isActive && (
                    <span className="badge">{t("client.versions.active")}</span>
                  )}
                  {pinned && (
                    <span className="badge">
                      <Pin size={10} />
                      {t("client.versions.pinned")}
                    </span>
                  )}
                  {client.latest?.guid === item.guid && (
                    <span className="badge">{t("client.versions.latest")}</span>
                  )}
                </strong>
                <small>
                  <code>{item.guid}</code> ·{" "}
                  {t("client.sizeMb", { size: item.sizeMb })} ·{" "}
                  {t("client.versions.installedAt", {
                    date: format.date(item.installedAt),
                  })}
                </small>
              </div>
              {!isActive && (
                <>
                  <Button
                    variant="outline"
                    size="sm"
                    onClick={() => onChange({ clientPinnedVersion: item.guid })}
                  >
                    <Pin />
                    {t("client.versions.use")}
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label={t("client.versions.remove", {
                      guid: item.guid,
                    })}
                    title={t("client.versions.removeTitle")}
                    disabled={busy}
                    onClick={() => {
                      if (
                        window.confirm(
                          t("client.versions.confirmRemove", {
                            guid: item.guid,
                          }),
                        )
                      )
                        actions.call(() => api.clientRemove(item.guid));
                    }}
                  >
                    <Trash2 />
                  </Button>
                </>
              )}
            </div>
          );
        })}
      </div>
      <div className="client-version-tools">
        {config.clientPinnedVersion && (
          <Button
            variant="outline"
            onClick={() => onChange({ clientPinnedVersion: null })}
          >
            {t("client.versions.followNewest")}
          </Button>
        )}
        <Button
          variant="outline"
          disabled={busy || client.installed.length < 2}
          onClick={() => actions.call(api.clientCleanup)}
        >
          <Trash2 />
          {t("client.versions.cleanup")}
        </Button>
      </div>
      <SettingRow
        title={t("client.versions.specific")}
        description={t("client.versions.specificDetail")}
      >
        <span className="client-specific">
          <input
            aria-label={t("client.versions.specificLabel")}
            placeholder="version-0123456789abcdef"
            spellCheck={false}
            value={specific}
            onChange={(event) => setSpecific(event.target.value.trim())}
          />
          <Button
            variant="outline"
            disabled={busy || !VERSION_PATTERN.test(specific)}
            onClick={() => actions.call(() => api.clientInstall(specific))}
          >
            <Download />
            {t("client.versions.install")}
          </Button>
        </span>
      </SettingRow>
    </SettingSection>
  );
}
