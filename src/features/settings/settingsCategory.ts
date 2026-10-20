export type SettingsCategory =
  "general" | "appearance" | "checks" | "launch" | "client" | "data";

// Same storage as SettingsView's `useUiPreference("settingsCategory")`.
const CATEGORY_KEY = "nocta.ui.settingsCategory";

/** Makes the settings view open on `category` the next time it is shown. */
export function preselectSettingsCategory(category: SettingsCategory) {
  try {
    localStorage.setItem(CATEGORY_KEY, JSON.stringify(category));
  } catch {
    /* the view opens on its last category */
  }
}
