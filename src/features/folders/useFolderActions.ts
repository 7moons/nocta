import { useCallback, useEffect, useRef, useState } from "react";
import type { Folder } from "@/generated/contracts";
import type { RunAction } from "@/lib/actions";
import { api } from "@/lib/ipc";
import { moveFolder, renameGroup } from "./folderTree";

/**
 * Folder changes shown at once and sent to the backend. The local copy gives way to the next
 * snapshot after the backend accepted the change, and goes back if it refused it.
 */
export function useFolderActions(saved: Folder[], run: RunAction) {
  const [local, setLocal] = useState<Folder[] | null>(null);
  // Set once the backend answered; the snapshot it publishes then replaces the local copy.
  const settled = useRef(false);
  const savedRef = useRef(saved);
  useEffect(() => {
    if (savedRef.current === saved) return;
    savedRef.current = saved;
    if (settled.current) setLocal(null);
  }, [saved]);
  const folders = local ?? saved;

  const change = useCallback(
    (next: Folder[] | null, request: () => Promise<unknown>) =>
      run(async () => {
        settled.current = false;
        const before = savedRef.current;
        if (next) setLocal(next);
        try {
          await request();
          settled.current = true;
          // The snapshot usually arrives before the answer.
          if (savedRef.current !== before) setLocal(null);
        } catch (failure) {
          setLocal(null);
          throw failure;
        }
      }),
    [run],
  );

  return {
    folders,
    create: (name: string, group: string | null, color: string) =>
      change(null, () => api.saveFolder(null, { name, color, group })),
    update: (
      folder: Folder,
      patch: Partial<Pick<Folder, "name" | "color">>,
    ) => {
      const next = { ...folder, ...patch };
      return change(
        folders.map((item) => (item.id === folder.id ? next : item)),
        () =>
          api.saveFolder(folder.id, {
            name: next.name,
            color: next.color,
            group: folder.group ?? null,
          }),
      );
    },
    move: (id: number, group: string | null, before: number | null) =>
      change(moveFolder(folders, id, group, before), () =>
        api.moveFolder(id, group, before),
      ),
    renameGroup: (from: string, to: string | null) =>
      change(renameGroup(folders, from, to), () =>
        api.renameFolderGroup(from, to),
      ),
    remove: (folder: Folder) =>
      change(
        folders.filter((item) => item.id !== folder.id),
        () => api.deleteFolder(folder.id),
      ),
  };
}

export type FolderActions = ReturnType<typeof useFolderActions>;
