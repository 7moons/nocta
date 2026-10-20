import { useRef } from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import type {
  AccountTask,
  ClientStage,
  GameInstance,
  InstanceStatus,
  TaskStatus,
} from "@/generated/contracts";
import { useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import { taskActive } from "@/lib/tasks";
import { ActivityLine, type Tone } from "./ActivityLine";
import { usePrivacy } from "@/lib/privacy";

export const ROW_HEIGHT = 34;

const TASK_TONES: Record<TaskStatus, Tone> = {
  queued: "muted",
  running: "active",
  waiting_user: "warning",
  cancelling: "warning",
  succeeded: "success",
  failed: "danger",
  cancelled: "muted",
};
const STAGE_TONES: Record<ClientStage, Tone> = {
  loading: "warning",
  joining: "warning",
  in_game: "success",
  disconnected: "danger",
};
export const INSTANCE_TONES: Record<InstanceStatus, Tone> = {
  starting: "warning",
  running: "success",
  exited: "muted",
  unknown: "danger",
};

type Props = { names: Map<number, string> } & (
  | { kind: "tasks"; rows: AccountTask[]; onAction: (id: number) => void }
  | { kind: "instances"; rows: GameInstance[]; onAction: (id: string) => void }
);

/** Virtualized task or game instance rows; the action cancels a task or closes a client. */
export function ActivityRows(props: Props) {
  const t = useT();
  const privacy = usePrivacy();
  const format = useFormat();
  const scroll = useRef<HTMLDivElement>(null);
  const virtual = useVirtualizer({
    count: props.rows.length,
    getScrollElement: () => scroll.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 5,
    getItemKey: (index) => props.rows[index].id,
  });
  const name = (accountId: number) =>
    props.names.get(accountId) ??
    t("accounts.bulk.unknownAccount", { id: String(accountId) });

  return (
    <div
      ref={scroll}
      className="activity-row-scroll"
      role="list"
      aria-label={t(
        props.kind === "tasks"
          ? "activity.tasksLabel"
          : "activity.instancesLabel",
      )}
    >
      <div style={{ height: virtual.getTotalSize(), position: "relative" }}>
        {virtual.getVirtualItems().map((item) => {
          const style = {
            position: "absolute" as const,
            top: 0,
            left: 0,
            width: "100%",
            height: item.size,
            transform: `translateY(${item.start}px)`,
          };
          if (props.kind === "tasks") {
            const task = props.rows[item.index];
            return (
              <ActivityLine
                key={item.key}
                style={style}
                status={t(`task.status.${task.status}`)}
                tone={TASK_TONES[task.status]}
                name={name(task.accountId)}
                what={t(`task.kind.${task.kind}`)}
                detail={task.detail}
                meta={
                  task.durationMs !== null &&
                  t("activity.seconds", {
                    seconds: (task.durationMs / 1000).toFixed(1),
                  })
                }
                action={
                  taskActive(task)
                    ? {
                        label: t("activity.cancelTask", {
                          id: String(task.id),
                        }),
                        run: () => props.onAction(task.id),
                      }
                    : undefined
                }
              />
            );
          }
          const instance = props.rows[item.index];
          // A running client shows what its own log says: loading, joining, in game, dropped.
          const live = instance.status === "running";
          return (
            <ActivityLine
              key={item.key}
              style={style}
              status={
                live
                  ? t(`activity.stage.${instance.stage}`)
                  : t(`activity.instanceStatus.${instance.status}`)
              }
              tone={
                live
                  ? STAGE_TONES[instance.stage]
                  : INSTANCE_TONES[instance.status]
              }
              name={privacy.nameOf(
                instance.accountId,
                instance.username || name(instance.accountId),
              )}
              what={t("launch.game.placeFallback", { id: instance.placeId })}
              whatTitle={t("activity.startedAt", {
                time: format.dateTime(instance.startedAt) ?? "",
              })}
              detail={privacy.scrub(
                instance.disconnect
                  ? t("activity.disconnected", { why: instance.disconnect })
                  : instance.detail,
              )}
              meta={
                instance.pid
                  ? [
                      t("activity.pid", { pid: String(instance.pid) }),
                      instance.memoryMb !== null &&
                        format.memory(instance.memoryMb),
                    ]
                      .filter(Boolean)
                      .join(" · ")
                  : t("activity.untracked")
              }
              action={
                instance.status !== "exited"
                  ? {
                      label: t("activity.closeInstance", {
                        name: instance.username,
                      }),
                      run: () => props.onAction(instance.id),
                    }
                  : undefined
              }
            />
          );
        })}
      </div>
    </div>
  );
}
