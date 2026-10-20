import type { Account, Folder } from "@/generated/contracts";

export interface FolderGroup {
  name: string;
  folders: Folder[];
}

export interface FolderLayout {
  ungrouped: Folder[];
  groups: FolderGroup[];
}

const byOrder = (a: Folder, b: Folder) =>
  a.sortOrder - b.sortOrder || a.id - b.id;

/** Ungrouped folders, then each group by name, every list in its saved order. */
export function layoutFolders(folders: Folder[]): FolderLayout {
  const ungrouped = folders.filter((folder) => !folder.group).sort(byOrder);
  const names = [
    ...new Set(folders.flatMap((folder) => folder.group ?? [])),
  ].sort((a, b) => a.localeCompare(b));
  return {
    ungrouped,
    groups: names.map((name) => ({
      name,
      folders: folders.filter((folder) => folder.group === name).sort(byOrder),
    })),
  };
}

/** Accounts per folder (`null` for accounts without one), Trash left out. */
export function countAccounts(accounts: Account[]) {
  const counts = new Map<number | null, number>();
  for (const account of accounts)
    if (!account.deletedAt)
      counts.set(account.folderId, (counts.get(account.folderId) ?? 0) + 1);
  return counts;
}

/** Renumbers one group's folders in the given order. */
function renumber(folders: Folder[], ordered: Folder[]) {
  const order = new Map(ordered.map((folder, index) => [folder.id, index]));
  return folders.map((folder) =>
    order.has(folder.id)
      ? { ...folder, sortOrder: order.get(folder.id)! }
      : folder,
  );
}

/** What the backend's `move_folder` does: into `group`, before `before` or last. */
export function moveFolder(
  folders: Folder[],
  id: number,
  group: string | null,
  before: number | null,
): Folder[] {
  const moving = folders.find((folder) => folder.id === id);
  if (!moving) return folders;
  const moved = { ...moving, group };
  const siblings = folders
    .filter((folder) => folder.id !== id && (folder.group ?? null) === group)
    .sort(byOrder);
  const index = siblings.findIndex((folder) => folder.id === before);
  siblings.splice(index < 0 ? siblings.length : index, 0, moved);
  return renumber(
    folders.map((folder) => (folder.id === id ? moved : folder)),
    siblings,
  );
}

/** What the backend's `rename_folder_group` does: the target's folders first, then the moved. */
export function renameGroup(
  folders: Folder[],
  from: string,
  to: string | null,
): Folder[] {
  const target = folders
    .filter((folder) => (folder.group ?? null) === to && folder.group !== from)
    .sort(byOrder);
  const moved = folders
    .filter((folder) => folder.group === from)
    .sort(byOrder)
    .map((folder) => ({ ...folder, group: to }));
  const ordered = [...target, ...moved];
  const updated = new Map(moved.map((folder) => [folder.id, folder]));
  return renumber(
    folders.map((folder) => updated.get(folder.id) ?? folder),
    ordered,
  );
}

/** Most colors the user can save; the backend allows as many. */
export const MAX_SAVED_COLORS = 32;

/** Colors offered in the folder menu. */
export const FOLDER_COLORS = [
  "#a3a3a3",
  "#e06c75",
  "#d19a66",
  "#e5c07b",
  "#98c379",
  "#56b6c2",
  "#61afef",
  "#c678dd",
] as const;

export const sameColor = (a: string, b: string) =>
  a.toLowerCase() === b.toLowerCase();
