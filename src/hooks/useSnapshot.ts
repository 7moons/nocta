import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import type { AppSnapshot } from "@/generated/contracts";
import { api, desktop } from "@/lib/ipc";
import { previewSnapshot } from "@/lib/preview";
import { errorMessage } from "@/lib/utils";

/**
 * A snapshot event. Accounts and logs are left out when they did not change since the last
 * event; the ones already shown stay, as the same objects, so nothing derived from them is
 * rebuilt.
 */
type SnapshotEvent = Omit<AppSnapshot, "accounts" | "logs"> &
  Partial<Pick<AppSnapshot, "accounts" | "logs">>;

export function useSnapshot() {
  const [snapshot, setSnapshot] = useState<AppSnapshot>(previewSnapshot);
  const [loading, setLoading] = useState(desktop);
  const [error, setError] = useState<string | null>(null);
  const revision = useRef(-1);
  // Whether a full snapshot from the desktop arrived; until then nothing can be kept.
  const complete = useRef(false);
  const refreshRef = useRef<() => Promise<void>>(async () => {});
  const accept = useCallback((next: SnapshotEvent) => {
    if (next.revision < revision.current) return;
    const partial = next.accounts === undefined || next.logs === undefined;
    if (partial && !complete.current) {
      void refreshRef.current();
      return;
    }
    revision.current = next.revision;
    if (!partial) complete.current = true;
    setSnapshot((previous) => ({
      ...next,
      accounts: next.accounts ?? previous.accounts,
      logs: next.logs ?? previous.logs,
    }));
    setLoading(false);
  }, []);
  const refresh = useCallback(async () => {
    if (!desktop) return;
    try {
      const full = await api.snapshot();
      // A full snapshot replaces everything, even after a newer partial event.
      revision.current = Math.min(revision.current, full.revision);
      accept(full);
      setError(null);
    } catch (error) {
      setError(errorMessage(error));
      setLoading(false);
    }
  }, [accept]);

  useEffect(() => {
    if (!desktop) return;
    let alive = true;
    let unlisten: (() => void) | undefined;
    void listen<SnapshotEvent>("nocta:snapshot", (event) => {
      if (alive) accept(event.payload);
    })
      .then(async (stop) => {
        if (!alive) {
          stop();
          return;
        }
        unlisten = stop;
        await refresh();
      })
      .catch((error) => {
        if (alive) {
          setError(errorMessage(error));
          setLoading(false);
        }
      });
    return () => {
      alive = false;
      unlisten?.();
    };
  }, [accept, refresh]);
  refreshRef.current = refresh;
  return { snapshot, loading, error, refresh };
}
