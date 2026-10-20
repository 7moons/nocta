import { useCallback, useEffect, useRef, useState } from "react";
import type { AppConfig, ClientState } from "@/generated/contracts";
import { api } from "@/lib/ipc";
import { errorMessage } from "@/lib/utils";

/**
 * Client install actions and their error. The latest version is looked up once per visit
 * (never in manual mode), so "update available" is current without polling.
 */
export function useClientActions(config: AppConfig, client: ClientState) {
  const [error, setError] = useState("");
  const [checking, setChecking] = useState(false);
  const call = useCallback((action: () => Promise<unknown>) => {
    setError("");
    void action().catch((reason) => setError(errorMessage(reason)));
  }, []);
  const check = useCallback(() => {
    setChecking(true);
    call(() => api.clientCheckLatest().finally(() => setChecking(false)));
  }, [call]);

  const checkOnVisit = useRef(
    !client.latest && config.clientUpdateMode !== "manual",
  );
  useEffect(() => {
    if (checkOnVisit.current) check();
  }, [check]);

  return { error, checking, call, check };
}

export type ClientActions = ReturnType<typeof useClientActions>;
