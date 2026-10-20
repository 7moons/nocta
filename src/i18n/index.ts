import { useSyncExternalStore } from "react";
import { en } from "./locales/en";
import type { Messages, MessageKey, Plural } from "./types";

export type { MessageKey } from "./types";

/**
 * Languages the interface ships with. Add a locale file and an entry here to add one. Only
 * English (the fallback) is built in; the others load when chosen, so the strings of seven
 * unused languages never sit in memory.
 */
export const LANGUAGES = [
  { id: "en", name: "English", load: async () => en },
  {
    id: "pl",
    name: "Polski",
    load: async () => (await import("./locales/pl")).pl,
  },
  {
    id: "de",
    name: "Deutsch",
    load: async () => (await import("./locales/de")).de,
  },
  {
    id: "es",
    name: "Español",
    load: async () => (await import("./locales/es")).es,
  },
  {
    id: "fr",
    name: "Français",
    load: async () => (await import("./locales/fr")).fr,
  },
  {
    id: "pt-BR",
    name: "Português (Brasil)",
    load: async () => (await import("./locales/ptBR")).ptBR,
  },
  {
    id: "tr",
    name: "Türkçe",
    load: async () => (await import("./locales/tr")).tr,
  },
  {
    id: "ru",
    name: "Русский",
    load: async () => (await import("./locales/ru")).ru,
  },
] as const satisfies readonly {
  id: string;
  name: string;
  load: () => Promise<Messages>;
}[];

export type LanguageId = (typeof LANGUAGES)[number]["id"];
export type TranslateParams = Record<string, string | number>;
export type Translate = (key: MessageKey, params?: TranslateParams) => string;

// The language is a per-profile interface preference, like the theme.
const STORAGE_KEY = "nocta.language.v1";
const listeners = new Set<() => void>();
const loaded = new Map<LanguageId, Messages>([["en", en]]);
// The chosen language; English is shown until its strings have loaded.
let wanted: LanguageId = load();
let current: LanguageId = "en";

async function ensure(language: LanguageId) {
  if (loaded.has(language)) return;
  const entry = LANGUAGES.find((item) => item.id === language);
  if (entry) loaded.set(language, await entry.load());
}

function isLanguage(value: unknown): value is LanguageId {
  return LANGUAGES.some((language) => language.id === value);
}

function load(): LanguageId {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (isLanguage(stored)) return stored;
  } catch {
    /* fall back to the system language */
  }
  // The system language: an exact match (pt-BR), else the same base language (pt-PT → pt-BR).
  const system = navigator.language;
  if (isLanguage(system)) return system;
  const base = system.slice(0, 2).toLowerCase();
  return (
    LANGUAGES.find((language) => language.id.slice(0, 2) === base)?.id ?? "en"
  );
}

function lookup(messages: Messages, key: string): string | Plural | undefined {
  let node: unknown = messages;
  for (const part of key.split(".")) {
    if (node === null || typeof node !== "object") return undefined;
    node = (node as Record<string, unknown>)[part];
  }
  return typeof node === "string" || (node && typeof node === "object")
    ? (node as string | Plural)
    : undefined;
}

const pluralRules = new Map<LanguageId, Intl.PluralRules>();
function pluralForm(language: LanguageId, count: number) {
  let rules = pluralRules.get(language);
  if (!rules) {
    rules = new Intl.PluralRules(language);
    pluralRules.set(language, rules);
  }
  return rules.select(count) as keyof Plural;
}

/** Translates `key` in `language`. Missing keys fall back to English, then to the key. */
export function translate(
  language: LanguageId,
  key: MessageKey,
  params?: TranslateParams,
): string {
  const messages = loaded.get(language) ?? en;
  const value = lookup(messages, key) ?? lookup(en, key);
  let text: string;
  if (value === undefined) text = key;
  else if (typeof value === "string") text = value;
  else {
    const count = Number(params?.count ?? 0);
    text = value[pluralForm(language, count)] ?? value.other;
  }
  if (!params) return text;
  return text.replace(/\{(\w+)\}/g, (match, name: string) => {
    const param = params[name];
    if (param === undefined) return match;
    return typeof param === "number" ? param.toLocaleString(language) : param;
  });
}

function apply() {
  document.documentElement.lang = current;
}

function show(language: LanguageId) {
  current = language;
  apply();
  for (const listener of listeners) listener();
}

export const languageStore = {
  /** Loads the saved language; resolves once it can be shown (English if it fails). */
  async init() {
    apply();
    try {
      await ensure(wanted);
      if (wanted !== current) show(wanted);
    } catch {
      /* stays in English */
    }
  },
  get: () => current,
  set(language: LanguageId) {
    wanted = language;
    try {
      localStorage.setItem(STORAGE_KEY, language);
    } catch {
      /* session only */
    }
    void ensure(language)
      .then(() => {
        // A later choice wins over one that is still loading.
        if (wanted === language && current !== language) show(language);
      })
      .catch(() => undefined);
  },
  subscribe(listener: () => void) {
    listeners.add(listener);
    return () => listeners.delete(listener);
  },
};

export function useLanguage() {
  return useSyncExternalStore(languageStore.subscribe, languageStore.get);
}

const translators = new Map<LanguageId, Translate>();
/** The translate function for the current language; components re-render when it changes. */
export function useT(): Translate {
  const language = useLanguage();
  let t = translators.get(language);
  if (!t) {
    t = (key, params) => translate(language, key, params);
    translators.set(language, t);
  }
  return t;
}

/** For code outside components, such as toasts raised by plain functions. */
export const t: Translate = (key, params) => translate(current, key, params);
