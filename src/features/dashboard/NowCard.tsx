import {
  CalendarClock,
  ListChecks,
  MemoryStick,
  Square,
  Workflow,
} from "lucide-react";
import { nextRun } from "@/features/automation/describe";
import type {
  AppSnapshot,
  SystemMemory,
  TaskKind,
} from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import { OverviewCard } from "./parts";
import type { Overview } from "./overview";

/** What Nocta does on its own and right now: memory, the next schedule, client care, tasks. */
export function NowCard({
  data,
  snapshot,
  memory,
  onAutomation,
  onStop,
}: {
  data: Overview;
  snapshot: AppSnapshot;
  memory: SystemMemory | null;
  onAutomation: () => void;
  onStop: () => void;
}) {
  const t = useT();
  const format = useFormat();
  const byKind = new Map<TaskKind, number>();
  for (const task of data.activeTasks)
    byKind.set(task.kind, (byKind.get(task.kind) ?? 0) + 1);
  const kinds = [...byKind.entries()].sort((a, b) => b[1] - a[1]);
  const busy = data.activeTasks.length > 0;
  const { config } = snapshot;
  const upcoming = config.schedules
    .map((schedule) => ({ schedule, at: nextRun(schedule) }))
    .filter((item) => item.at !== null)
    .sort((a, b) => a.at!.getTime() - b.at!.getTime())[0];
  const preset =
    upcoming &&
    config.launchPresets.find((item) => item.id === upcoming.schedule.presetId);
  const care = [
    config.antiAfk && t("dashboard.now.antiAfk"),
    config.autoRejoin && t("dashboard.now.rejoin"),
    config.keepAwake && t("dashboard.now.keepAwake"),
    config.launchPacing === "in_game" && t("dashboard.now.pacing"),
  ].filter((item): item is string => !!item);
  return (
    <OverviewCard
      className="overview-now"
      title={t("dashboard.now.title")}
      action={
        busy && (
          <Button variant="outline" size="sm" onClick={onStop}>
            <Square />
            {t("accounts.stop")}
          </Button>
        )
      }
    >
      <ul className="overview-rows">
        {memory && <MemoryRow memory={memory} />}
        <li>
          <CalendarClock size={15} />
          <span>
            <strong>{t("dashboard.now.next")}</strong>
            <small>
              {upcoming?.at
                ? t("dashboard.now.nextRun", {
                    name: preset?.name ?? "?",
                    time: format.dateTime(upcoming.at.toISOString()) ?? "",
                  })
                : t("dashboard.now.noSchedule")}
            </small>
          </span>
          <Button variant="ghost" size="sm" onClick={onAutomation}>
            {t("dashboard.open")}
          </Button>
        </li>
        <li>
          <Workflow size={15} />
          <span>
            <strong>{t("dashboard.now.care")}</strong>
            <small>
              {care.length ? care.join(" · ") : t("dashboard.now.careOff")}
            </small>
          </span>
        </li>
        <li>
          <ListChecks size={15} />
          <span>
            <strong>
              {t("dashboard.now.tasks", { count: data.activeTasks.length })}
            </strong>
            <small>
              {kinds.length
                ? kinds
                    .map(
                      ([kind, count]) => `${t(`task.kind.${kind}`)} ${count}`,
                    )
                    .join(" · ")
                : t("dashboard.now.noTasks")}
            </small>
          </span>
        </li>
      </ul>
    </OverviewCard>
  );
}

/** Share of the commit limit at which more clients may fail to start. */
const COMMIT_WARNING = 0.85;

/**
 * RAM and commit (RAM plus page file) of the computer. With RAM capped per client, the commit
 * limit is what stops more clients from starting, so it warns before it runs out.
 */
function MemoryRow({ memory }: { memory: SystemMemory }) {
  const t = useT();
  const format = useFormat();
  const ramUsed = memory.ramTotalMb - memory.ramFreeMb;
  const commitShare = memory.commitUsedMb / Math.max(1, memory.commitLimitMb);
  const tight = commitShare >= COMMIT_WARNING;
  return (
    <li className={tight ? "memory-tight" : undefined}>
      <MemoryStick size={15} />
      <span>
        <strong>{t("dashboard.now.memory")}</strong>
        <small>
          {t("dashboard.now.memoryDetail", {
            ram: `${format.memory(ramUsed)} / ${format.memory(memory.ramTotalMb)}`,
            commit: `${format.memory(memory.commitUsedMb)} / ${format.memory(memory.commitLimitMb)}`,
          })}
        </small>
        <span
          className="memory-meter"
          role="meter"
          aria-label={t("dashboard.now.memoryCommit")}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round(commitShare * 100)}
        >
          <span style={{ width: `${Math.min(100, commitShare * 100)}%` }} />
        </span>
        {tight && (
          <small className="memory-warning">
            {t("dashboard.now.memoryTight")}
          </small>
        )}
      </span>
    </li>
  );
}
