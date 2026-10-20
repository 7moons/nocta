import { useState } from "react";
import { Archive, RotateCcw } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { BackupDialog } from "../BackupDialog";
import { LegacyImport } from "../LegacyImport";
import { SettingSection } from "../SettingControls";

export function DataSettings() {
  const t = useT();
  const [backupMode, setBackupMode] = useState<"export" | "restore" | null>(
    null,
  );
  return (
    <>
      <SettingSection
        id="backup"
        title={t("settings.data.backup")}
        description={t("settings.data.backupDetail")}
      >
        <div className="backup-actions">
          <Button variant="outline" onClick={() => setBackupMode("export")}>
            <Archive size={12} />
            {t("settings.data.export")}
          </Button>
          <Button variant="outline" onClick={() => setBackupMode("restore")}>
            <RotateCcw size={12} />
            {t("settings.data.restore")}
          </Button>
        </div>
      </SettingSection>
      <SettingSection
        id="legacy"
        title={t("settings.data.legacy")}
        description={t("settings.data.legacyDetail")}
      >
        <LegacyImport />
      </SettingSection>
      <BackupDialog mode={backupMode} onClose={() => setBackupMode(null)} />
    </>
  );
}
