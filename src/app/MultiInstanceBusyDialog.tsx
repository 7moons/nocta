import { useState } from "react";
import { LoaderCircle, MonitorX } from "lucide-react";
import type { AppSnapshot } from "@/generated/contracts";
import { Dialog } from "@/components/ui/dialog";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";

// Multi-instance needs Roblox's singleton lock. A client that is already running holds it
// ("busy"), so offer to close every Roblox client and take the lock in one step.
export function MultiInstanceBusyDialog({
  snapshot,
  onCloseAll,
}: {
  snapshot: AppSnapshot;
  onCloseAll: () => Promise<void>;
}) {
  const t = useT();
  const busy =
    snapshot.launcher.multiInstanceSupported &&
    snapshot.launcher.multiInstance === "busy";
  const [dismissed, setDismissed] = useState(false);
  const [working, setWorking] = useState(false);
  // Ask again the next time the lock turns busy, not while the user already said no.
  if (!busy && dismissed) setDismissed(false);
  const running =
    snapshot.instances.filter((instance) => instance.status !== "exited")
      .length + snapshot.externalInstances.length;
  return (
    <Dialog
      open={busy && !dismissed}
      onOpenChange={(open) => {
        if (!open && !working) setDismissed(true);
      }}
      title={t("multiBusy.title")}
      description={t("multiBusy.subtitle")}
    >
      <div className="dialog-body">
        <div className="notice-card">
          <MonitorX size={18} />
          <p>
            {running > 0
              ? t("multiBusy.running", { count: running })
              : t("multiBusy.noClients")}
          </p>
        </div>
        <div className="dialog-footer">
          <Button
            variant="outline"
            disabled={working}
            onClick={() => setDismissed(true)}
          >
            {t("multiBusy.notNow")}
          </Button>
          {/* With nothing to close, the same call simply retries taking the lock. */}
          <Button
            variant={running > 0 ? "destructive" : "default"}
            disabled={working}
            onClick={() => {
              setWorking(true);
              void onCloseAll().finally(() => setWorking(false));
            }}
          >
            {working && <LoaderCircle className="spin" />}
            {t(running > 0 ? "multiBusy.closeAll" : "multiBusy.retry")}
          </Button>
        </div>
      </div>
    </Dialog>
  );
}
