import type { DragEvent } from "react";

/** A folder being reordered; the data is its ID. */
export const FOLDER_DRAG = "application/nocta-folder";
/** Accounts dragged out of the accounts table; the data is a JSON list of IDs. */
export const ACCOUNTS_DRAG = "application/nocta-accounts";

export type DropPosition = "before" | "after" | "into";

export const carries = (event: DragEvent, type: string) =>
  event.dataTransfer.types.includes(type);

/** Above or below the middle of the row under the pointer. */
export function halfOf(event: DragEvent<HTMLElement>): "before" | "after" {
  const box = event.currentTarget.getBoundingClientRect();
  return event.clientY < box.top + box.height / 2 ? "before" : "after";
}

export function draggedFolder(event: DragEvent) {
  const id = Number(event.dataTransfer.getData(FOLDER_DRAG));
  return Number.isInteger(id) && id > 0 ? id : null;
}

export function draggedAccounts(event: DragEvent): number[] {
  try {
    const ids: unknown = JSON.parse(event.dataTransfer.getData(ACCOUNTS_DRAG));
    return Array.isArray(ids)
      ? ids.filter((id): id is number => Number.isInteger(id))
      : [];
  } catch {
    return [];
  }
}

/** Shows `label` next to the pointer while dragging instead of a picture of the row. */
export function setDragImage(event: DragEvent, label: string) {
  const badge = document.createElement("div");
  badge.className = "drag-badge";
  badge.textContent = label;
  document.body.append(badge);
  event.dataTransfer.setDragImage(badge, -8, -8);
  // The browser takes its picture right away; the element is not needed after that.
  requestAnimationFrame(() => badge.remove());
}
