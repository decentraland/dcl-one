// The profiles this browser has already resolved (address → name and face), kept across reloads so a known
// creator's name paints at once instead of flipping from an address to the name when the profile request lands.
// The request still runs in the background and refreshes what is kept. Bounded and short-lived: a week, at most
// MAX entries, newest kept; anything unreadable is treated as empty, and a face is only ever an http(s) URL.

import type { CreatorProfile } from "../hooks";

export const PROFILE_CACHE_KEY = "dcl:shop:profiles:v1";
const TTL_MS = 7 * 24 * 60 * 60 * 1000;
const MAX = 500;

type Entry = CreatorProfile & { at: number };

let memory: Map<string, Entry> | null = null;
let saveTimer: ReturnType<typeof setTimeout> | null = null;

const isFace = (picture: unknown): picture is string | null => picture === null || (typeof picture === "string" && /^https?:\/\//.test(picture));

function load(): Map<string, Entry> {
  if (memory) return memory;
  memory = new Map();
  try {
    const raw = typeof localStorage === "undefined" ? null : localStorage.getItem(PROFILE_CACHE_KEY);
    const parsed = raw ? (JSON.parse(raw) as Record<string, Partial<Entry>>) : {};
    const now = Date.now();
    for (const [address, entry] of Object.entries(parsed)) {
      if (!entry || typeof entry.name !== "string" || typeof entry.at !== "number" || now - entry.at >= TTL_MS) continue;
      if (!isFace(entry.picture ?? null)) continue;
      const claimed = typeof entry.claimed === "boolean" ? entry.claimed : null;
      memory.set(address, { name: entry.name, picture: entry.picture ?? null, claimed, at: entry.at });
    }
  } catch {
    // Blocked or corrupt storage: start empty, the requests fill it again.
  }
  return memory;
}

/** Keeps the newest MAX in memory as in storage. */
function trim(map: Map<string, Entry>): void {
  if (map.size <= MAX) return;
  const oldest = [...map.entries()].sort((a, b) => a[1].at - b[1].at).slice(0, map.size - MAX);
  for (const [address] of oldest) map.delete(address);
}

/** Writes what is kept now, instead of on the next tick. */
export function flushProfileCache(): void {
  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = null;
  if (!memory) return;
  try {
    localStorage.setItem(PROFILE_CACHE_KEY, JSON.stringify(Object.fromEntries(memory)));
  } catch {
    // Full or blocked storage: the in-memory copy still serves this page.
  }
}

/** One write per tick, however many profile answers land in it. */
function queueSave(): void {
  if (saveTimer !== null) return;
  saveTimer = setTimeout(flushProfileCache, 0);
}

/** The profile last resolved for an address, if this browser has one younger than a week. */
export function cachedProfile(address: string | null | undefined): CreatorProfile | null {
  if (!address) return null;
  const entry = load().get(address.toLowerCase());
  if (!entry) return null;
  const { at: _, ...profile } = entry;
  return profile;
}

/** Keeps freshly resolved profiles; an address that came back without a profile is forgotten. */
export function rememberProfiles(addresses: string[], profiles: Record<string, CreatorProfile | null | undefined>): void {
  const map = load();
  const now = Date.now();
  for (const raw of addresses) {
    const address = raw.toLowerCase();
    const profile = profiles[address];
    if (profile && isFace(profile.picture)) map.set(address, { name: profile.name, picture: profile.picture, claimed: profile.claimed, at: now });
    else map.delete(address);
  }
  trim(map);
  queueSave();
}

/**
 * Tests only: forget the in-memory copy, and the stored one too unless `keepStorage` (a reload: the next read goes
 * back to storage).
 */
export function resetProfileCache({ keepStorage = false }: { keepStorage?: boolean } = {}): void {
  memory = null;
  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = null;
  if (keepStorage) return;
  try {
    localStorage.removeItem(PROFILE_CACHE_KEY);
  } catch {
    // No storage in this environment: nothing to clear.
  }
}
