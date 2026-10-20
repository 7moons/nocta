import { useMemo } from "react";
import { ChevronDown, ChevronUp, Terminal } from "lucide-react";
import type {
  Account,
  AccountTask,
  ExternalInstance,
  GameInstance,
  LogEntry,
  LogPreferences,
} from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useUiPreference } from "@/hooks/useUiPreference";
import { useT } from "@/i18n";
import { taskActive } from "@/lib/tasks";
import { ActivityRows } from "./activity/ActivityRows";
import {
  InstancesView,
  InstanceTools,
  runningOwned,
} from "./activity/InstancesView";
import {
  LogColumns,
  LogList,
  LogTools,
  useLogFilters,
} from "./activity/LogView";
import { usePanelHeight } from "./activity/usePanelHeight";
import { usePrivacy } from "@/lib/privacy";

const TABS = ["logs", "tasks", "instances"] as const;
type Tab = (typeof TABS)[number];

// Below this window height the panel starts folded, leaving room for the accounts.
const OPEN_BY_DEFAULT_MIN_HEIGHT = 800;

/** The bottom panel: logs, account tasks and running Roblox clients. */
export function ActivityPanel({
  logs,
  tasks,
  accounts,
  onCancel,
  instances,
  externalInstances,
  onCloseInstance,
  onCloseExternal,
  onCloseAll,
  onCloseAllRoblox,
  onArrange,
  preferences,
  onPreferences,
}: {
  logs: LogEntry[];
  tasks: AccountTask[];
  accounts: Account[];
  onCancel: (id: number) => void;
  instances: GameInstance[];
  externalInstances: ExternalInstance[];
  onCloseInstance: (id: string) => void;
  onCloseExternal: (id: string) => void;
  onCloseAll: () => void;
  onCloseAllRoblox: () => void;
  onArrange: () => void;
  preferences: LogPreferences;
  onPreferences: (preferences: LogPreferences) => void;
}) {
  const t = useT();
  const privacy = usePrivacy();
  const names = useMemo(
    () =>
      new Map(
        accounts.map((account) => [account.id, privacy.username(account)]),
      ),
    [accounts, privacy],
  );
  const [open, setOpen] = useUiPreference(
    "activityOpen",
    window.innerHeight >= OPEN_BY_DEFAULT_MIN_HEIGHT,
  );
  const [storedTab, setTab] = useUiPreference<string>("activityTab", "logs");
  const tab: Tab = TABS.find((item) => item === storedTab) ?? "logs";
  const size = usePanelHeight();
  const filters = useLogFilters(logs, preferences, onPreferences, names);
  // Newest first.
  const reversedTasks = useMemo(() => [...tasks].reverse(), [tasks]);
  const reversedInstances = useMemo(
    () => [...instances].reverse(),
    [instances],
  );
  const owned = runningOwned(instances);
  const counts: Record<Tab, number> = {
    logs: logs.length,
    tasks: tasks.filter(taskActive).length,
    instances: owned + externalInstances.length,
  };

  return (
    <section
      className={`activity-panel ${open ? "open" : "collapsed"}`}
      aria-label={t("activity.label")}
      style={open ? { height: size.height } : undefined}
    >
      {open && (
        <div
          className="activity-resize"
          role="separator"
          aria-orientation="horizontal"
          aria-label={t("activity.resize")}
          title={t("activity.resize")}
          onPointerDown={size.startResize}
          onDoubleClick={size.reset}
        />
      )}
      <header className="activity-header">
        <div className="panel-tabs">
          {TABS.map((value) => (
            <button
              key={value}
              className={tab === value ? "active" : ""}
              onClick={() => {
                setTab(value);
                setOpen(true);
              }}
            >
              {value === "logs" && <Terminal size={12} />}
              {t(`activity.tabs.${value}`)}
              <span>{counts[value]}</span>
            </button>
          ))}
        </div>
        <div className="activity-tools">
          {open && tab === "instances" && (
            <InstanceTools
              owned={owned}
              total={counts.instances}
              onCloseOwned={onCloseAll}
              onCloseAll={onCloseAllRoblox}
              onArrange={onArrange}
            />
          )}
          {open && tab === "logs" && <LogTools filters={filters} />}
          <Button
            variant="ghost"
            size="icon"
            aria-label={t(open ? "activity.collapse" : "activity.expand")}
            onClick={() => setOpen(!open)}
          >
            {open ? <ChevronDown /> : <ChevronUp />}
          </Button>
        </div>
      </header>
      {open && tab === "logs" && filters.columnsOpen && (
        <LogColumns filters={filters} />
      )}
      {open && (
        <div className="activity-content">
          {tab === "logs" ? (
            <LogList filters={filters} names={names} />
          ) : tab === "tasks" && tasks.length > 0 ? (
            <ActivityRows
              kind="tasks"
              rows={reversedTasks}
              names={names}
              onAction={onCancel}
            />
          ) : tab === "instances" &&
            (instances.length > 0 || externalInstances.length > 0) ? (
            <InstancesView
              instances={reversedInstances}
              external={externalInstances}
              names={names}
              onClose={onCloseInstance}
              onCloseExternal={onCloseExternal}
            />
          ) : (
            <div className="panel-empty">
              <span className="status-dot" />
              <span>
                {t(
                  tab === "tasks"
                    ? "activity.noTasks"
                    : "activity.instances.none",
                )}
              </span>
            </div>
          )}
        </div>
      )}
    </section>
  );
}
