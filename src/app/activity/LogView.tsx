import { useMemo, useState } from "react";
import { Search, Terminal, X } from "lucide-react";
import type { LogEntry, LogPreferences } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import { usePrivacy } from "@/lib/privacy";

const LEVELS = ["INFO", "WARNING", "SUCCESS", "ERROR", "DEBUG"];
const COLUMNS = [
  ["showTimestamp", "time"],
  ["showAccount", "account"],
  ["showModule", "service"],
  ["showLevel", "level"],
] as const;

/**
 * Log filters and visible columns. Preference changes show right away and are saved through
 * `onSave`; the saved value takes over once it comes back.
 */
export function useLogFilters(
  logs: LogEntry[],
  saved: LogPreferences,
  onSave: (preferences: LogPreferences) => void,
  names: Map<number, string>,
) {
  const [draft, setDraft] = useState<{
    base: LogPreferences;
    value: LogPreferences;
  } | null>(null);
  const preferences = draft?.base === saved ? draft.value : saved;
  const [search, setSearch] = useState("");
  const [columnsOpen, setColumnsOpen] = useState(false);
  const modules = useMemo(
    () => [...new Set(logs.map((log) => log.module))].sort(),
    [logs],
  );
  const shown = useMemo(() => {
    const needle = search.toLowerCase();
    return logs.filter(
      (log) =>
        (preferences.level === "all" || log.level === preferences.level) &&
        (preferences.module === "all" || log.module === preferences.module) &&
        `${log.message} ${log.module} ${log.level} ${names.get(log.accountId ?? -1) ?? ""}`
          .toLowerCase()
          .includes(needle),
    );
  }, [logs, search, preferences, names]);
  return {
    preferences,
    modules,
    shown,
    search,
    setSearch,
    columnsOpen,
    setColumnsOpen,
    filtered:
      !!search || preferences.level !== "all" || preferences.module !== "all",
    change(patch: Partial<LogPreferences>) {
      const value = { ...preferences, ...patch };
      setDraft({ base: saved, value });
      onSave(value);
    },
  };
}

type LogFilters = ReturnType<typeof useLogFilters>;

export function LogTools({ filters }: { filters: LogFilters }) {
  const t = useT();
  const { preferences, change } = filters;
  return (
    <>
      <select
        aria-label={t("activity.logs.severity")}
        value={preferences.level}
        onChange={(event) => change({ level: event.target.value })}
      >
        <option value="all">{t("activity.logs.allLevels")}</option>
        {LEVELS.map((level) => (
          <option key={level}>{level}</option>
        ))}
      </select>
      <select
        aria-label={t("activity.logs.service")}
        value={preferences.module}
        onChange={(event) => change({ module: event.target.value })}
      >
        <option value="all">{t("activity.logs.allServices")}</option>
        {[
          ...new Set([
            ...filters.modules,
            ...(preferences.module === "all" ? [] : [preferences.module]),
          ]),
        ].map((module) => (
          <option key={module}>{module}</option>
        ))}
      </select>
      <Button
        variant="ghost"
        aria-expanded={filters.columnsOpen}
        onClick={() => filters.setColumnsOpen(!filters.columnsOpen)}
      >
        {t("activity.logs.columns")}
      </Button>
      <label className="search-field small">
        <Search size={12} />
        <input
          aria-label={t("activity.logs.search")}
          placeholder={t("activity.logs.searchPlaceholder")}
          value={filters.search}
          onChange={(event) => filters.setSearch(event.target.value)}
        />
        {filters.search && (
          <button
            aria-label={t("activity.logs.clearSearch")}
            onClick={() => filters.setSearch("")}
          >
            <X size={11} />
          </button>
        )}
      </label>
    </>
  );
}

export function LogColumns({ filters }: { filters: LogFilters }) {
  const t = useT();
  return (
    <div className="log-columns">
      {COLUMNS.map(([key, label]) => (
        <label key={key}>
          <input
            type="checkbox"
            checked={filters.preferences[key]}
            onChange={(event) =>
              filters.change({ [key]: event.target.checked })
            }
          />
          {t(`activity.logs.column.${label}`)}
        </label>
      ))}
    </div>
  );
}

export function LogList({
  filters,
  names,
}: {
  filters: LogFilters;
  names: Map<number, string>;
}) {
  const t = useT();
  const privacy = usePrivacy();
  const format = useFormat();
  const { preferences } = filters;
  if (filters.shown.length === 0)
    return (
      <div className="panel-empty">
        <Terminal size={16} />
        <span>
          {t(
            filters.filtered
              ? "activity.logs.noMatches"
              : "activity.logs.empty",
          )}
        </span>
      </div>
    );
  return (
    <>
      {filters.shown.map((log) => (
        <div className={`log-line log-${log.level.toLowerCase()}`} key={log.id}>
          {preferences.showTimestamp && (
            <time>{format.time(log.timestamp)}</time>
          )}
          {preferences.showLevel && (
            <span className="log-level">{log.level.toLowerCase()}</span>
          )}
          {preferences.showModule && (
            <span className="log-module">{log.module}</span>
          )}
          {preferences.showAccount && (
            <span className="log-account">
              {log.accountId === null
                ? "—"
                : (names.get(log.accountId) ??
                  t("accounts.bulk.unknownAccount", {
                    id: String(log.accountId),
                  }))}
            </span>
          )}
          <span className="log-message">{privacy.scrub(log.message)}</span>
        </div>
      ))}
    </>
  );
}
