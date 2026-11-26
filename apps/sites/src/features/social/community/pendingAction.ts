import type { CommunityAction } from "./view";

/**
 * The join this tab asked for before it went to sign in. A link carrying `?action=` alone runs
 * nothing: only a return whose nonce matches a fresh marker this tab wrote does, once.
 */
const KEY = "dcl:community-action";
export const PENDING_ACTION_TTL_MS = 10 * 60_000;

type Marker = { communityId: string; action: CommunityAction; nonce: string; ts: number };

/** The tab's session storage, or null where it is unavailable (private mode, blocked site data). */
function tabStorage(): Storage | null {
  try {
    return typeof window === "undefined" ? null : window.sessionStorage;
  } catch {
    return null;
  }
}

function newNonce(): string {
  return typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
    ? crypto.randomUUID()
    : `${Date.now().toString(36)}${Math.random().toString(36).slice(2)}`;
}

/** Records the action before sign-in; its nonce goes in the return URL. Null when it cannot be recorded. */
export function rememberAction(
  communityId: string,
  action: CommunityAction,
  storage: Storage | null = tabStorage(),
  now = Date.now(),
): string | null {
  if (!storage) return null;
  const marker: Marker = { communityId, action, nonce: newNonce(), ts: now };
  try {
    storage.setItem(KEY, JSON.stringify(marker));
    return marker.nonce;
  } catch {
    return null;
  }
}

/**
 * Whether this return was started here: the marker matches the community, action and nonce and is
 * younger than the TTL. The marker is removed whatever the answer, so it is used at most once.
 */
export function takeAction(
  communityId: string,
  action: CommunityAction,
  nonce: string | null,
  storage: Storage | null = tabStorage(),
  now = Date.now(),
): boolean {
  if (!storage || !nonce) return false;
  try {
    const raw = storage.getItem(KEY);
    storage.removeItem(KEY);
    if (!raw) return false;
    const marker = JSON.parse(raw) as Partial<Marker> | null;
    return (
      marker?.communityId === communityId &&
      marker.action === action &&
      marker.nonce === nonce &&
      typeof marker.ts === "number" &&
      now - marker.ts >= 0 &&
      now - marker.ts <= PENDING_ACTION_TTL_MS
    );
  } catch {
    return false;
  }
}
