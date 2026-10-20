import { useSyncExternalStore } from "react";

/** How times are shown: as the language usually writes them, or always 12- or 24-hour. */
export type Clock = "auto" | "12" | "24";

export const CLOCKS: readonly Clock[] = ["auto", "12", "24"];

// An interface preference of this profile, like the language and the theme.
const STORAGE_KEY = "nocta.clock.v1";
const listeners = new Set<() => void>();
let current: Clock = load();

function load(): Clock {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (CLOCKS.includes(stored as Clock)) return stored as Clock;
  } catch {
    /* the language decides */
  }
  return "auto";
}

export const clockStore = {
  get: () => current,
  set(clock: Clock) {
    current = clock;
    try {
      localStorage.setItem(STORAGE_KEY, clock);
    } catch {
      /* session only */
    }
    for (const listener of listeners) listener();
  },
  subscribe(listener: () => void) {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },
};

export function useClock() {
  return useSyncExternalStore(clockStore.subscribe, clockStore.get);
}

/** The Intl options that make times follow `clock`. */
export function hourCycle(clock: Clock): Intl.DateTimeFormatOptions {
  if (clock === "12") return { hourCycle: "h12" };
  if (clock === "24") return { hourCycle: "h23" };
  return {};
}
