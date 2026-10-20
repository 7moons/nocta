import { useSyncExternalStore } from "react";
import { BUILT_IN_THEMES, DEFAULT_THEME_ID, builtInTheme } from "./presets";
import { applyTheme, normalizeTheme, type Theme } from "./theme";

// Themes are a per-profile UI preference, stored next to the other window
// preferences. Storage failures keep the current session themed.
const STORAGE_KEY = "nocta.theme.v1";

interface ThemeState {
  activeId: string;
  custom: Theme[];
  // Unsaved theme shown while the editor is open; never persisted.
  preview: Theme | null;
}

let state: ThemeState = load();
const listeners = new Set<() => void>();

function load(): ThemeState {
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null") as {
      activeId?: unknown;
      custom?: unknown;
    } | null;
    const custom = Array.isArray(stored?.custom)
      ? stored.custom
          .map((item) =>
            normalizeTheme(item, (item as { id?: unknown })?.id as string),
          )
          .filter(
            (theme): theme is Theme =>
              theme !== null &&
              theme.id.startsWith("custom-") &&
              !builtInTheme(theme.id),
          )
      : [];
    const activeId =
      typeof stored?.activeId === "string" ? stored.activeId : DEFAULT_THEME_ID;
    return { activeId, custom, preview: null };
  } catch {
    return { activeId: DEFAULT_THEME_ID, custom: [], preview: null };
  }
}

function persist() {
  try {
    localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify({ activeId: state.activeId, custom: state.custom }),
    );
  } catch {
    /* session only */
  }
}

function findTheme(id: string) {
  return builtInTheme(id) ?? state.custom.find((theme) => theme.id === id);
}

function activeTheme() {
  return findTheme(state.activeId) ?? builtInTheme(DEFAULT_THEME_ID)!;
}

function update(next: Partial<ThemeState>, save = true) {
  state = { ...state, ...next };
  if (save) persist();
  applyTheme(state.preview ?? activeTheme());
  for (const listener of listeners) listener();
}

export const themeStore = {
  init() {
    applyTheme(activeTheme());
  },
  select(id: string) {
    if (findTheme(id)) update({ activeId: id });
  },
  // Inserts or replaces a custom theme. Built-in themes are read-only.
  save(theme: Theme, activate = true) {
    if (theme.builtIn || builtInTheme(theme.id)) return;
    const exists = state.custom.some((item) => item.id === theme.id);
    const custom = exists
      ? state.custom.map((item) => (item.id === theme.id ? theme : item))
      : [...state.custom, theme];
    update({ custom, activeId: activate ? theme.id : state.activeId });
  },
  remove(id: string) {
    update({
      custom: state.custom.filter((theme) => theme.id !== id),
      activeId: state.activeId === id ? DEFAULT_THEME_ID : state.activeId,
    });
  },
  preview(theme: Theme | null) {
    update({ preview: theme }, false);
  },
};

let snapshot = { ...state, active: activeTheme(), themes: allThemes() };
function allThemes() {
  return [...BUILT_IN_THEMES, ...state.custom];
}
function getSnapshot() {
  if (
    snapshot.activeId !== state.activeId ||
    snapshot.custom !== state.custom ||
    snapshot.preview !== state.preview
  )
    snapshot = { ...state, active: activeTheme(), themes: allThemes() };
  return snapshot;
}
function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useThemes() {
  return useSyncExternalStore(subscribe, getSnapshot);
}
