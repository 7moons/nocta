import { useEffect, useMemo, useState } from "react";
import { ArrowRight, Gauge } from "lucide-react";
import type { AppSnapshot, SystemMemory } from "@/generated/contracts";
import type { View } from "@/app/navigation";
import { Button } from "@/components/ui/button";
import { ViewHeader } from "@/components/ViewHeader";
import { preselectSettingsCategory } from "@/features/settings/settingsCategory";
import { useT } from "@/i18n";
import type { RunAction } from "@/lib/actions";
import { api, desktop } from "@/lib/ipc";
import { FoldersCard } from "./FoldersCard";
import { HealthCard } from "./HealthCard";
import { NowCard } from "./NowCard";
import { overview } from "./overview";
import { PerformanceCard } from "./PerformanceCard";
import { PlayingCard } from "./PlayingCard";
import { SetupCard } from "./SetupCard";
import { SummaryTiles } from "./SummaryTiles";

// "Checked in the last 24 hours" is recounted this often while the view is open.
const CLOCK_MS = 60_000;
// The computer's memory is read again this often while the view is open.
const MEMORY_MS = 10_000;

/** RAM and commit of the computer, read every few seconds; null outside the desktop app. */
function useSystemMemory() {
  const [memory, setMemory] = useState<SystemMemory | null>(null);
  useEffect(() => {
    if (!desktop) return;
    let alive = true;
    const read = () =>
      api
        .systemMemory()
        .then((value) => alive && setMemory(value))
        .catch(() => undefined);
    void read();
    const timer = window.setInterval(read, MEMORY_MS);
    return () => {
      alive = false;
      window.clearInterval(timer);
    };
  }, []);
  return memory;
}

/**
 * Who is playing first: accounts in a game, their clients and Robux, then what Nocta does on
 * its own, then account health, folders, setup that is still missing, provider balances and
 * long-term success rates. Every number leads to the place where it can be acted on.
 */
export function DashboardView({
  snapshot,
  run,
  onAccounts,
  onFolder,
  onView,
  onAdd,
}: {
  snapshot: AppSnapshot;
  run: RunAction;
  /** Opens the accounts, optionally filtered to one session state. */
  onAccounts: (health?: string) => void;
  onFolder: (id: number | null) => void;
  onView: (view: View) => void;
  onAdd: () => void;
}) {
  const t = useT();
  const [now, setNow] = useState(Date.now);
  useEffect(() => {
    const timer = window.setInterval(() => setNow(Date.now()), CLOCK_MS);
    return () => window.clearInterval(timer);
  }, []);
  const data = useMemo(() => overview(snapshot, now), [snapshot, now]);
  const memory = useSystemMemory();
  const sessionsReady = snapshot.runtime.capabilities.includes("sessions");

  return (
    <>
      <ViewHeader
        icon={Gauge}
        title={t("views.dashboard")}
        subtitle={t("dashboard.subtitle")}
        actions={
          <Button variant="outline" onClick={() => onAccounts()}>
            {t("dashboard.openAccounts")}
            <ArrowRight />
          </Button>
        }
      />
      <div className="standard-content overview">
        <SummaryTiles
          data={data}
          onAccounts={onAccounts}
          onPlaying={() =>
            document
              .querySelector(".overview-playing")
              ?.scrollIntoView({ behavior: "smooth" })
          }
          onActivity={() => onAccounts()}
        />
        <div className="overview-grid">
          <PlayingCard
            data={data}
            now={now}
            onLaunch={() => onAccounts()}
            onArrange={() =>
              run(api.arrangeWindows, (count) =>
                t("activity.arranged", { count }),
              )
            }
            onClose={(id) => run(() => api.closeInstance(id))}
          />
          <NowCard
            data={data}
            snapshot={snapshot}
            memory={memory}
            onAutomation={() => onView("automation")}
            onStop={() => run(api.stopTasks)}
          />
          <SetupCard
            snapshot={snapshot}
            actions={{
              onAdd,
              onAccounts: () => onAccounts(),
              onSettings: (category) => {
                preselectSettingsCategory(category);
                onView("settings");
              },
            }}
          />
          <HealthCard
            data={data}
            canCheck={sessionsReady}
            onOpen={(health) => onAccounts(health)}
            onCheckUnchecked={() =>
              run(
                () =>
                  api.startSessionTasks(
                    data.accounts
                      .filter((account) => !account.checkedAt)
                      .map((account) => account.id),
                    "check_health",
                  ),
                t("dashboard.health.checking", { count: data.neverChecked }),
              )
            }
          />
          <FoldersCard data={data} onFolder={onFolder} />
          <PerformanceCard metrics={snapshot.metrics} />
        </div>
      </div>
    </>
  );
}
