import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";
import { t } from "@/i18n";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

/** The message of an IPC or JavaScript error, for showing to the user. */
export function errorMessage(error: unknown): string {
  if (
    error &&
    typeof error === "object" &&
    "message" in error &&
    typeof error.message === "string"
  )
    return error.message;
  return typeof error === "string" ? error : t("errors.generic");
}
