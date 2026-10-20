import type { PointerEvent } from "react";
import { useUiPreference } from "@/hooks/useUiPreference";

const MIN_HEIGHT = 110;
// The panel never takes more than this share of the window, so the view above stays usable.
const MAX_SHARE = 0.6;
const DEFAULT_HEIGHT = 210;

const clamp = (height: number) =>
  Math.round(
    Math.min(Math.max(height, MIN_HEIGHT), window.innerHeight * MAX_SHARE),
  );

/** The activity panel's height, changed by dragging its top edge and remembered. */
export function usePanelHeight() {
  const [stored, setStored] = useUiPreference("activityHeight", DEFAULT_HEIGHT);
  const height = clamp(stored);

  function startResize(event: PointerEvent<HTMLElement>) {
    event.preventDefault();
    const handle = event.currentTarget;
    handle.setPointerCapture(event.pointerId);
    const startY = event.clientY;
    const startHeight = height;
    let next = startHeight;
    const panel = handle.parentElement;
    const move = (moved: globalThis.PointerEvent) => {
      next = clamp(startHeight + startY - moved.clientY);
      // Applied directly while dragging; saved once on release.
      if (panel) panel.style.height = `${next}px`;
    };
    const stop = () => {
      handle.removeEventListener("pointermove", move);
      handle.removeEventListener("pointerup", stop);
      handle.removeEventListener("pointercancel", stop);
      setStored(next);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", stop);
    handle.addEventListener("pointercancel", stop);
  }

  return { height, startResize, reset: () => setStored(DEFAULT_HEIGHT) };
}
