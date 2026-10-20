import { useCallback, useEffect, useState } from "react";
import type { RunAction } from "@/lib/actions";
import { errorMessage } from "@/lib/utils";

export interface Notice {
  message: string;
  error: boolean;
}

const SHOW_MS = 3500;
const SHOW_ERROR_MS = 10_000;

/**
 * The toast notification and the count of operations still running. `run` performs an
 * action, then shows `success` (or the message built from its result) or the error.
 */
export function useNotices() {
  const [pending, setPending] = useState(0);
  const [notice, setNotice] = useState<Notice | null>(null);
  const run: RunAction = useCallback(
    <T>(
      action: () => Promise<T>,
      success?: string | ((result: T) => string),
    ) => {
      setPending((value) => value + 1);
      return action()
        .then((result) => {
          const message =
            typeof success === "function" ? success(result) : success;
          if (message) setNotice({ message, error: false });
        })
        .catch((failure) =>
          setNotice({ message: errorMessage(failure), error: true }),
        )
        .finally(() => setPending((value) => value - 1));
    },
    [],
  );
  useEffect(() => {
    if (!notice) return;
    const timer = window.setTimeout(
      () => setNotice(null),
      notice.error ? SHOW_ERROR_MS : SHOW_MS,
    );
    return () => clearTimeout(timer);
  }, [notice]);
  return {
    pending,
    notice,
    run,
    showError: useCallback(
      (message: string) => setNotice({ message, error: true }),
      [],
    ),
    dismiss: () => setNotice(null),
  };
}
