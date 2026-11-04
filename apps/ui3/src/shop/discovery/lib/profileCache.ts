// The profiles this browser has already resolved (address → name and face), kept across reloads so a known
// creator's name paints at once instead of flipping from an address to the name when the profile request lands.
// The request still runs in the background and refreshes what is kept. Bounded and short-lived: a week, at most
// MAX entries, newest kept; anything unreadable is treated as empty.

import type { CreatorProfile } from "../hooks";

const KEY = "dcl:shop:profiles:v1";
const TTL_MS = 7 * 24 * 60 * 60 * 1000;
const MAX = 500;

type Entry = CreatorProfile & { at: number };

let memory: Map<string, Entry> | null = null;

function load(): Map<string, Entry> {
  if (memory) return memory;
  memory = new Map();
  try {
    const raw = typeof localStorage === "undefined" ? null : localStorage.getItem(KEY);
    const parsed = raw ? (JSON.parse(raw) as Record<string, Entry>) : {};
    const now = Date.now();
    for (const [address, entry] of Object.entries(parsed)) {
      if (entry && typeof entry.name === "string" && typeof entry.at === "number" && now - entry.at < TTL_MS) memory.set(address, entry);
    }
  } catch {
    // Blocked or corrupt storage: start empty, the requests fill it again.
  }
  return memory;
}

function save(map: Map<string, Entry>): void {
  try {
    const newest = [...map.entries()].sort((a, b) => b[1].at - a[1].at).slice(0, MAX);
    localStorage.setItem(KEY, JSON.stringify(Object.fromEntries(newest)));
  } catch {
    // Full or blocked storage: the in-memory copy still serves this page.
  }
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
    if (profile) map.set(address, { ...profile, at: now });
    else map.delete(address);
  }
  save(map);
}

/** Tests only: drop the in-memory copy so the next read goes back to storage. */
export function resetProfileCache(): void {
  memory = null;
}
