// Every themeable color. Each entry becomes a CSS custom property (`--${name}`);
// stylesheets reference only these, so a theme is fully described by this list.
// Labels and hints for the theme editor live in the translations: `theme.token.<camelCase name>`.
export const THEME_TOKENS = [
  // Surfaces, from the window background up to selected items.
  { name: "bg", group: "surfaces" },
  { name: "surface-sunken", group: "surfaces" },
  { name: "surface", group: "surfaces" },
  { name: "surface-overlay", group: "surfaces" },
  { name: "surface-hover", group: "surfaces" },
  { name: "surface-raised", group: "surfaces" },
  { name: "surface-selected", group: "surfaces" },
  { name: "control", group: "surfaces" },
  { name: "control-hover", group: "surfaces" },

  { name: "border-subtle", group: "borders" },
  { name: "border", group: "borders" },
  { name: "border-strong", group: "borders" },
  { name: "border-emphasis", group: "borders" },
  { name: "focus-ring", group: "borders" },

  { name: "text-strong", group: "text" },
  { name: "text", group: "text" },
  { name: "text-secondary", group: "text" },
  { name: "text-muted", group: "text" },
  { name: "text-subtle", group: "text" },
  { name: "text-faint", group: "text" },

  { name: "accent", group: "accent" },
  { name: "accent-foreground", group: "accent" },

  { name: "success", group: "status" },
  { name: "success-surface", group: "status" },
  { name: "success-border", group: "status" },
  { name: "warning", group: "status" },
  { name: "danger", group: "status" },
  { name: "danger-surface", group: "status" },
  { name: "danger-border", group: "status" },
  { name: "danger-solid", group: "status" },
  { name: "danger-solid-foreground", group: "status" },

  { name: "overlay", group: "effects" },
  { name: "shadow", group: "effects" },
  { name: "scrollbar", group: "effects" },
] as const;

export type ThemeTokenName = (typeof THEME_TOKENS)[number]["name"];
export type ThemeTokens = Record<ThemeTokenName, string>;

export const TOKEN_GROUPS = [
  "surfaces",
  "borders",
  "text",
  "accent",
  "status",
  "effects",
] as const;

export type TokenGroup = (typeof TOKEN_GROUPS)[number];

type CamelCase<S extends string> = S extends `${infer Head}-${infer Tail}`
  ? `${Head}${Capitalize<CamelCase<Tail>>}`
  : S;

/** `surface-sunken` → `surfaceSunken`, the token's key in the translations. */
export const tokenKey = <Name extends ThemeTokenName>(name: Name) =>
  name.replace(/-([a-z])/g, (_, letter: string) =>
    letter.toUpperCase(),
  ) as CamelCase<Name>;
