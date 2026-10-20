import type { PresetTarget, Schedule } from "@/generated/contracts";
import type { Translate } from "@/i18n";
import { STRATEGY_KEYS } from "@/features/accounts/launch/target";

/** Weekdays in schedule order (0 = Monday). */
export const WEEKDAYS = [0, 1, 2, 3, 4, 5, 6] as const;

/** Short weekday name in the current language, Monday first. */
export function weekdayName(locale: string, day: number) {
  // 2024-01-01 was a Monday.
  return new Intl.DateTimeFormat(locale, { weekday: "short" }).format(
    new Date(2024, 0, 1 + day),
  );
}

export function describeTarget(t: Translate, target: PresetTarget) {
  switch (target.kind) {
    case "place":
      return t("automation.target.placeSummary", { id: target.placeId });
    case "link":
      return t("automation.target.linkSummary");
    case "pick":
      return t("automation.target.pickSummary", {
        id: target.placeId,
        strategy: t(`launch.join.${STRATEGY_KEYS[target.strategy]}`),
      });
    case "follow":
      return t("launch.join.followSummary", { name: target.username });
  }
}

export function describeDays(t: Translate, locale: string, days: number[]) {
  if (days.length === 7) return t("automation.schedule.everyDay");
  const sorted = [...days].sort();
  if (sorted.join() === "0,1,2,3,4") return t("automation.schedule.weekdays");
  if (sorted.join() === "5,6") return t("automation.schedule.weekends");
  return sorted.map((day) => weekdayName(locale, day)).join(", ");
}

/** The next time a schedule runs, as a local Date, or null when it is off. */
export function nextRun(schedule: Schedule, now = new Date()) {
  if (!schedule.enabled || !schedule.days.length) return null;
  const [hours, minutes] = schedule.time.split(":").map(Number);
  for (let offset = 0; offset <= 7; offset++) {
    const day = new Date(now);
    day.setDate(now.getDate() + offset);
    day.setHours(hours, minutes, 0, 0);
    const weekday = (day.getDay() + 6) % 7;
    if (schedule.days.includes(weekday) && day > now) return day;
  }
  return null;
}
