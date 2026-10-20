import type { CSSProperties } from "react";
import { THEME_TOKENS, type ThemeTokens } from "./tokens";
import { defaultThemeFor } from "./presets";
import { t } from "@/i18n";

export type ColorScheme = "dark" | "light";

export interface Theme {
  id: string;
  name: string;
  scheme: ColorScheme;
  tokens: ThemeTokens;
  builtIn?: boolean;
}

const EXPORT_FORMAT = "nocta-theme";
const EXPORT_VERSION = 1;

export function isColor(value: string) {
  return typeof CSS !== "undefined" && CSS.supports("color", value.trim());
}

export function newThemeId() {
  const random =
    typeof crypto.randomUUID === "function"
      ? crypto.randomUUID()
      : `${Date.now().toString(36)}${Math.random().toString(36).slice(2)}`;
  return `custom-${random}`;
}

// Accepts anything that might be a theme (storage, clipboard) and returns a complete one.
// Invalid or missing tokens fall back to the built-in theme of the same scheme.
export function normalizeTheme(input: unknown, id?: string): Theme | null {
  if (!input || typeof input !== "object") return null;
  const raw = input as Record<string, unknown>;
  const scheme: ColorScheme = raw.scheme === "light" ? "light" : "dark";
  const fallback = defaultThemeFor(scheme).tokens;
  const sourceTokens =
    raw.tokens && typeof raw.tokens === "object"
      ? (raw.tokens as Record<string, unknown>)
      : {};
  const tokens = {} as ThemeTokens;
  for (const { name } of THEME_TOKENS) {
    const value = sourceTokens[name];
    tokens[name] =
      typeof value === "string" && isColor(value)
        ? value.trim()
        : fallback[name];
  }
  const name =
    typeof raw.name === "string" && raw.name.trim()
      ? raw.name.trim().slice(0, 40)
      : "Imported theme";
  const themeId = id ?? (typeof raw.id === "string" ? raw.id : newThemeId());
  return { id: themeId, name, scheme, tokens };
}

export function exportTheme(theme: Theme) {
  return JSON.stringify(
    {
      format: EXPORT_FORMAT,
      version: EXPORT_VERSION,
      name: theme.name,
      scheme: theme.scheme,
      tokens: theme.tokens,
    },
    null,
    2,
  );
}

export function parseThemeExport(text: string): Theme {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error(t("theme.error.json"));
  }
  if (
    !parsed ||
    typeof parsed !== "object" ||
    (parsed as { format?: unknown }).format !== EXPORT_FORMAT
  )
    throw new Error(t("theme.error.notTheme"));
  const theme = normalizeTheme(parsed, newThemeId());
  if (!theme) throw new Error(t("theme.error.unreadable"));
  return theme;
}

// Custom properties for a theme, usable as an inline style to scope it to one element.
export function themeStyle(theme: Pick<Theme, "tokens" | "scheme">) {
  const style: Record<string, string> = { colorScheme: theme.scheme };
  for (const { name } of THEME_TOKENS) style[`--${name}`] = theme.tokens[name];
  return style as CSSProperties;
}

export function applyTheme(theme: Theme, root = document.documentElement) {
  for (const { name } of THEME_TOKENS)
    root.style.setProperty(`--${name}`, theme.tokens[name]);
  root.style.colorScheme = theme.scheme;
  root.dataset.theme = theme.id;
  document
    .querySelector('meta[name="theme-color"]')
    ?.setAttribute("content", theme.tokens.bg);
}

// <input type="color"> only understands #rrggbb; alpha is kept separately so
// picking a new hue does not drop the transparency of tokens like the backdrop.
export function splitColor(value: string): { hex: string; alpha: string } {
  const hex = /^#([0-9a-f]{3,4}|[0-9a-f]{6}|[0-9a-f]{8})$/i.exec(value.trim());
  if (hex) {
    let digits = hex[1];
    if (digits.length <= 4)
      digits = [...digits].map((digit) => digit + digit).join("");
    return { hex: `#${digits.slice(0, 6)}`, alpha: digits.slice(6) };
  }
  const context = document.createElement("canvas").getContext("2d");
  if (!context) return { hex: "#000000", alpha: "" };
  context.fillStyle = value;
  const normalized = context.fillStyle;
  if (normalized.startsWith("#")) return { hex: normalized, alpha: "" };
  const parts = /rgba?\(([^)]+)\)/.exec(normalized)?.[1].split(",") ?? [];
  const [r, g, b, a] = parts.map((part) => Number(part.trim()));
  const byte = (channel: number) =>
    Math.round(channel).toString(16).padStart(2, "0");
  return {
    hex: `#${byte(r || 0)}${byte(g || 0)}${byte(b || 0)}`,
    alpha: a === undefined || a >= 1 ? "" : byte(a * 255),
  };
}
