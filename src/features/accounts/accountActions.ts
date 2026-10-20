/** Where restored accounts go: their old folder, the general space or a chosen folder ID. */
export type RestoreDestination = "original" | "general" | `${number}`;

/** The backend action and folder for a restore destination. */
export function restoreAction(
  destination: RestoreDestination,
): [action: "restore" | "restore_unfiled", folderId: number | null] {
  if (destination === "original") return ["restore", null];
  if (destination === "general") return ["restore_unfiled", null];
  return ["restore", Number(destination)];
}

/** Bulk session operations offered for selected accounts. */
export const SESSION_ACTIONS = [
  "validate_cookie",
  "open_browser",
  "refresh_cookie",
] as const;
export type SessionAction = (typeof SESSION_ACTIONS)[number];
