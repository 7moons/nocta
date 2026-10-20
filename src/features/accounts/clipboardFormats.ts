import { useSyncExternalStore } from "react";
import type { Translate } from "@/i18n";
import { isTemplateError, parseTemplate } from "./clipboardTemplate";

/** A named clipboard template (see clipboardTemplate.ts). Built-in ones cannot be edited. */
export interface ClipboardFormat {
  id: string;
  name: string;
  template: string;
  builtIn?: boolean;
}

type BuiltInName =
  | "username"
  | "password"
  | "cookie"
  | "userPass"
  | "userCookie"
  | "userPassCookie"
  | "userRandom"
  | "userRandomCookie";

/** Built-in formats show a translated name (`clipboard.builtIn.*`); custom ones their own. */
export function formatName(format: ClipboardFormat, t: Translate) {
  const builtIn = BUILT_IN_FORMATS.find((item) => item.id === format.id);
  return builtIn ? t(`clipboard.builtIn.${builtIn.nameKey}`) : format.name;
}

// IDs are stored as the selected format, so they never change.
export const BUILT_IN_FORMATS: readonly (ClipboardFormat & {
  nameKey: BuiltInName;
})[] = (
  [
    ["username", "username", "{username}"],
    ["password", "password", "{password}"],
    ["cookie", "cookie", "{cookie}"],
    ["user:pass", "userPass", "{username}:{password}"],
    ["user:cookie", "userCookie", "{username}:{cookie}"],
    ["user:pass:cookie", "userPassCookie", "{username}:{password}:{cookie}"],
    ["user:randompass", "userRandom", "{username}:{random}"],
    ["user:random:cookie", "userRandomCookie", "{username}:{random}:{cookie}"],
  ] as const
).map(([id, nameKey, template]) => ({
  id,
  nameKey,
  name: nameKey,
  template,
  builtIn: true,
}));

const STORAGE_KEY = "nocta.clipboardFormats.v1";
interface State {
  selectedId: string;
  custom: ClipboardFormat[];
}
let state: State = load();
const listeners = new Set<() => void>();

function load(): State {
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null");
    const custom = Array.isArray(stored?.custom)
      ? stored.custom.filter(
          (item: ClipboardFormat) =>
            typeof item?.id === "string" &&
            typeof item.name === "string" &&
            typeof item.template === "string" &&
            !isTemplateError(parseTemplate(item.template)),
        )
      : [];
    return {
      selectedId:
        typeof stored?.selectedId === "string" ? stored.selectedId : "username",
      custom,
    };
  } catch {
    return { selectedId: "username", custom: [] };
  }
}
function update(next: Partial<State>) {
  state = { ...state, ...next };
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    /* session only */
  }
  for (const listener of listeners) listener();
}

export const clipboardFormatStore = {
  select(id: string) {
    update({ selectedId: id });
  },
  save(format: ClipboardFormat) {
    const exists = state.custom.some((item) => item.id === format.id);
    update({
      custom: exists
        ? state.custom.map((item) => (item.id === format.id ? format : item))
        : [...state.custom, format],
      selectedId: format.id,
    });
  },
  remove(id: string) {
    update({
      custom: state.custom.filter((item) => item.id !== id),
      selectedId: state.selectedId === id ? "username" : state.selectedId,
    });
  },
};

// Rebuilt only when the stored state object changes, so readers get a stable value.
let snapshot = build(state);
function build(source: State) {
  const formats = [...BUILT_IN_FORMATS, ...source.custom];
  return {
    source,
    formats,
    selected:
      formats.find((format) => format.id === source.selectedId) ?? formats[0],
  };
}
function subscribe(listener: () => void) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
function getSnapshot() {
  if (snapshot.source !== state) snapshot = build(state);
  return snapshot;
}
export function useClipboardFormats() {
  return useSyncExternalStore(subscribe, getSnapshot);
}
