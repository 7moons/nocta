import { useMemo } from "react";
import { hourCycle, useClock, type Clock } from "./clock";
import { useLanguage, type LanguageId } from "./index";

const MINUTE = 60_000;

function parse(value: string | null | undefined) {
  if (!value) return null;
  const time = Date.parse(value);
  return Number.isFinite(time) ? new Date(time) : null;
}

/** Dates, times and numbers formatted for one language, with times in the chosen clock. */
export function formatter(language: LanguageId, clock: Clock = "auto") {
  const cycle = hourCycle(clock);
  const short = new Intl.DateTimeFormat(language, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
    ...cycle,
  });
  const full = new Intl.DateTimeFormat(language, {
    dateStyle: "medium",
    timeStyle: "short",
    ...cycle,
  });
  const time = new Intl.DateTimeFormat(language, {
    timeStyle: "medium",
    ...cycle,
  });
  const clockTime = new Intl.DateTimeFormat(language, {
    timeStyle: "short",
    ...cycle,
  });
  const relative = new Intl.RelativeTimeFormat(language, {
    numeric: "auto",
    style: "narrow",
  });
  const number = new Intl.NumberFormat(language);
  const gigabytes = new Intl.NumberFormat(language, {
    minimumFractionDigits: 1,
    maximumFractionDigits: 1,
  });
  return {
    /** "Oct 5, 14:03", or "—" without a date. */
    date: (value: string | null | undefined) => {
      const date = parse(value);
      return date ? short.format(date) : "—";
    },
    dateTime: (value: string | null | undefined) => {
      const date = parse(value);
      return date ? full.format(date) : null;
    },
    time: (value: string | null | undefined) => {
      const date = parse(value);
      return date ? time.format(date) : "";
    },
    /** A time of day saved as "HH:MM", such as a schedule's: "14:30" or "2:30 PM". */
    clock: (value: string) => {
      const [hours, minutes] = value.split(":").map(Number);
      if (!Number.isInteger(hours) || !Number.isInteger(minutes)) return value;
      return clockTime.format(new Date(2024, 0, 1, hours, minutes));
    },
    /** "5 min ago"; null without a date. */
    relative: (value: string | null | undefined, now: number) => {
      const date = parse(value);
      if (!date) return null;
      const minutes = Math.floor(Math.max(0, now - date.getTime()) / MINUTE);
      if (minutes < 60) return relative.format(-minutes, "minute");
      if (minutes < 1440)
        return relative.format(-Math.floor(minutes / 60), "hour");
      return relative.format(-Math.floor(minutes / 1440), "day");
    },
    number: (value: number) => number.format(value),
    /** "850 MB" or "1.4 GB". */
    memory: (megabytes: number) =>
      megabytes < 1024
        ? `${number.format(megabytes)} MB`
        : `${gigabytes.format(megabytes / 1024)} GB`,
  };
}

export type Formatter = ReturnType<typeof formatter>;

export function useFormat(): Formatter {
  const language = useLanguage();
  const clock = useClock();
  return useMemo(() => formatter(language, clock), [language, clock]);
}
