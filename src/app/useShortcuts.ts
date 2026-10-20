import { useEffect } from "react";
import type { Location, View } from "./navigation";

const VIEW_KEYS: Record<string, View> = {
  "1": "accounts",
  "2": "dashboard",
  "4": "settings",
  "5": "friends",
  "6": "automation",
  ",": "settings",
};

/**
 * Browser-style navigation: Alt+arrows and the mouse back/forward buttons, plus Ctrl+1–6 for
 * the main views and Ctrl+, for settings.
 */
export function useShortcuts(
  back: () => void,
  forward: () => void,
  navigate: (patch: Partial<Location>) => void,
) {
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.altKey && event.key === "ArrowLeft") back();
      else if (event.altKey && event.key === "ArrowRight") forward();
      else if (
        (event.ctrlKey || event.metaKey) &&
        !event.altKey &&
        !event.shiftKey &&
        VIEW_KEYS[event.key]
      )
        navigate({ view: VIEW_KEYS[event.key] });
      else return;
      event.preventDefault();
    };
    const onMouse = (event: MouseEvent) => {
      if (event.button === 3) back();
      else if (event.button === 4) forward();
      else return;
      event.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    window.addEventListener("mouseup", onMouse);
    return () => {
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("mouseup", onMouse);
    };
  }, [back, forward, navigate]);
}
