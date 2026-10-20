import { useEffect, useSyncExternalStore } from "react";
import { api, desktop } from "./ipc";

/**
 * Roblox headshots by user ID. Rows ask for their avatar as they render; requests are
 * batched, and each ID is fetched at most once per app run (`null` means it has none).
 */
const known = new Map<string, string | null>();
const queued = new Set<string>();
const listeners = new Set<() => void>();
let flush: ReturnType<typeof setTimeout> | undefined;
let version = 0;

const BATCH = 100;

function changed() {
  version++;
  listeners.forEach((listener) => listener());
}

async function load() {
  flush = undefined;
  const ids = [...queued].slice(0, BATCH);
  ids.forEach((id) => queued.delete(id));
  if (queued.size) flush = setTimeout(() => void load(), 0);
  try {
    const urls = await api.userAvatars(ids);
    ids.forEach((id) => known.set(id, urls[id] ?? null));
  } catch {
    ids.forEach((id) => known.set(id, null));
  }
  changed();
}

function request(userId: string) {
  if (!desktop || known.has(userId) || queued.has(userId)) return;
  queued.add(userId);
  flush ??= setTimeout(() => void load(), 30);
}

const subscribe = (listener: () => void) => {
  listeners.add(listener);
  return () => listeners.delete(listener);
};

/** The headshot URL of a Roblox user, once loaded. */
export function useAvatar(userId: string | null | undefined) {
  useSyncExternalStore(subscribe, () => version);
  useEffect(() => {
    if (userId) request(userId);
  }, [userId]);
  return userId ? (known.get(userId) ?? null) : null;
}
