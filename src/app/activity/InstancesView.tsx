import type { ExternalInstance, GameInstance } from "@/generated/contracts";
import { LayoutGrid } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import { ActivityLine } from "./ActivityLine";
import { ActivityRows } from "./ActivityRows";

export const runningOwned = (instances: GameInstance[]) =>
  instances.filter((instance) => instance.status !== "exited").length;

/** "Close" buttons for the instances tab header. */
export function InstanceTools({
  owned,
  total,
  onCloseOwned,
  onCloseAll,
  onArrange,
}: {
  owned: number;
  total: number;
  onCloseOwned: () => void;
  onCloseAll: () => void;
  onArrange: () => void;
}) {
  const t = useT();
  return (
    <>
      {owned > 0 && (
        <Button variant="outline" onClick={onArrange}>
          <LayoutGrid />
          {t("activity.arrange")}
        </Button>
      )}
      {owned > 0 && (
        <Button variant="outline" onClick={onCloseOwned}>
          {t("activity.instances.closeOwned")}
        </Button>
      )}
      {total > 0 && (
        <Button
          variant="destructive"
          onClick={() => {
            if (
              window.confirm(
                t("activity.instances.confirmCloseAll", { count: total }),
              )
            )
              onCloseAll();
          }}
        >
          {t("multiBusy.closeAll")}
        </Button>
      )}
    </>
  );
}

/** Roblox clients started outside Nocta, then the ones Nocta started for accounts. */
export function InstancesView({
  instances,
  external,
  names,
  onClose,
  onCloseExternal,
}: {
  instances: GameInstance[];
  external: ExternalInstance[];
  names: Map<number, string>;
  onClose: (id: string) => void;
  onCloseExternal: (id: string) => void;
}) {
  const t = useT();
  const format = useFormat();
  return (
    <div className="instances-view">
      {external.length > 0 && (
        <div
          className="external-instances"
          role="list"
          aria-label={t("activity.instances.externalLabel")}
        >
          <p className="external-instances-heading">
            {t("activity.instances.externalHeading", {
              count: external.length,
            })}
          </p>
          {external.map((instance) => {
            const pid = String(instance.pid);
            return (
              <ActivityLine
                key={instance.id}
                status={t("activity.instances.external")}
                tone="warning"
                name={t("activity.instances.client")}
                what={t("activity.startedAt", {
                  time: instance.startedAt
                    ? format.time(instance.startedAt)
                    : t("activity.instances.beforeNocta"),
                })}
                detail={t("activity.instances.notLinked")}
                meta={t("activity.pid", { pid })}
                action={{
                  label: t("activity.instances.closeExternal", { pid }),
                  title: t("activity.instances.closeExternalTitle"),
                  run: () => {
                    if (
                      window.confirm(
                        t("activity.instances.confirmCloseExternal", { pid }),
                      )
                    )
                      onCloseExternal(instance.id);
                  },
                }}
              />
            );
          })}
        </div>
      )}
      {instances.length > 0 && (
        <ActivityRows
          kind="instances"
          rows={instances}
          names={names}
          onAction={onClose}
        />
      )}
    </div>
  );
}
