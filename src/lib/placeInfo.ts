import { useEffect, useState } from "react";
import type { PlaceInfo } from "@/generated/contracts";
import { api } from "@/lib/ipc";

// Game names and icons barely change, so they are shared by every component and kept
// across restarts. This keeps Roblox lookups to one per game per day, well clear of
// rate limits. Failed lookups are remembered briefly so they are not retried in a loop.
const STORAGE_KEY = "nocta.placeInfo.v1";
const TTL_MS = 24 * 60 * 60 * 1000;
const FAILURE_TTL_MS = 2 * 60 * 1000;
const MAX_ENTRIES = 200;

interface Entry {
  info: PlaceInfo | null;
  at: number;
}

const cache = new Map<string, Entry>(load());
const inflight = new Map<string, Promise<PlaceInfo | null>>();

function load(): [string, Entry][] {
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "[]");
    return Array.isArray(stored)
      ? stored.filter(
          (item): item is [string, Entry] =>
            Array.isArray(item) &&
            typeof item[0] === "string" &&
            !!item[1]?.info &&
            Date.now() - item[1].at < TTL_MS,
        )
      : [];
  } catch {
    return [];
  }
}

function persist() {
  try {
    // Only successful lookups are stored; the newest entries win when trimming.
    const entries = [...cache.entries()]
      .filter(([, entry]) => entry.info)
      .sort((a, b) => b[1].at - a[1].at)
      .slice(0, MAX_ENTRIES);
    localStorage.setItem(STORAGE_KEY, JSON.stringify(entries));
  } catch {
    /* cache stays in memory */
  }
}

function fresh(entry: Entry | undefined) {
  return (
    !!entry && Date.now() - entry.at < (entry.info ? TTL_MS : FAILURE_TTL_MS)
  );
}

export function cachedPlaceInfo(placeId: string) {
  const entry = cache.get(placeId);
  return fresh(entry) ? (entry?.info ?? null) : null;
}

// Search results already carry the name and icon, so picking one needs no extra lookup.
export function rememberPlaceInfo(info: PlaceInfo) {
  cache.set(info.placeId, { info, at: Date.now() });
  persist();
}

export function getPlaceInfo(placeId: string): Promise<PlaceInfo | null> {
  if (!placeId) return Promise.resolve(null);
  const entry = cache.get(placeId);
  if (fresh(entry)) return Promise.resolve(entry!.info);
  const pending = inflight.get(placeId);
  if (pending) return pending;
  const request = api
    .placeInfo(placeId)
    .then((info) => info)
    .catch(() => null)
    .then((info) => {
      cache.set(placeId, { info, at: Date.now() });
      inflight.delete(placeId);
      if (info) persist();
      return info;
    });
  inflight.set(placeId, request);
  return request;
}

export function usePlaceInfo(placeId: string | null | undefined) {
  const id = placeId ?? "";
  const [info, setInfo] = useState(() => cachedPlaceInfo(id));
  useEffect(() => {
    let active = true;
    setInfo(cachedPlaceInfo(id));
    void getPlaceInfo(id).then((value) => {
      if (active) setInfo(value);
    });
    return () => {
      active = false;
    };
  }, [id]);
  return info;
}
