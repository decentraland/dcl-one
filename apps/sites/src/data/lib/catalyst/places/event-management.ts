import { queryOptions } from "@tanstack/react-query";
import { z } from "zod";
import type { AuthIdentity } from "../../auth/types";
import { CatalystError, postJSON, signedGetJSON } from "../client";
import { apiOkOf } from "../envelope";
import { EventRecordSchema } from "../generated-schemas/events";

export const EVENT_PERMISSIONS = [
  { id: "approve_own_event", label: "Approve own hangouts" },
  { id: "approve_any_event", label: "Approve any hangout" },
  { id: "edit_any_event", label: "Edit any hangout" },
  { id: "edit_any_schedule", label: "Manage schedules" },
  { id: "edit_any_profile", label: "Legacy profile permission" },
] as const;
export type EventPermission = typeof EVENT_PERMISSIONS[number]["id"];
export type ManagedEvent = z.infer<typeof EventRecordSchema>;
const Profile = z.object({ user: z.string(), permissions: z.array(z.string()) });
export type EventProfile = z.infer<typeof Profile>;
const EventList = apiOkOf(z.union([z.array(EventRecordSchema), z.object({ events: z.array(EventRecordSchema), total: z.number() })])).extend({ total: z.number().optional() });

export async function loadEventProfile(identity: AuthIdentity, signal?: AbortSignal): Promise<EventProfile> {
  const raw = await signedGetJSON("/events/api/profiles/me/settings", { identity, signal });
  const profile = apiOkOf(Profile).parse(raw).data;
  if (profile.user.toLowerCase() !== identity.signer.toLowerCase()) throw new Error("The event settings belong to another account. Sign in again.");
  return profile;
}

export async function loadOwnedEvents(identity: AuthIdentity, options: { offset: number; limit: number; search?: string }, signal?: AbortSignal) {
  const raw = await signedGetJSON("/events/api/events", {
    identity, signal,
    query: { list: "all", owner: true, limit: options.limit + 1, offset: options.offset, search: options.search || undefined, order: "desc" },
  });
  return eventPage(raw, options.offset, options.limit);
}

/**
 * Every event the signer created, page after page: the list comes newest first, so its soonest upcoming
 * event can sit on any page and a single page could hide it.
 */
export async function loadAllOwnedEvents(identity: AuthIdentity, signal?: AbortSignal): Promise<ManagedEvent[]> {
  const limit = 100;
  const owned = new Map<string, ManagedEvent>();
  for (let offset = 0; ; offset += limit) {
    const { events, hasMore } = await loadOwnedEvents(identity, { offset, limit }, signal);
    const previousCount = owned.size;
    for (const event of events) owned.set(event.id, event);
    if (!hasMore || events.length === 0) break;
    if (owned.size === previousCount) throw new Error("Your events list did not advance. Retry to reload it.");
  }
  return [...owned.values()];
}

function eventPage(raw: unknown, offset: number, limit: number) {
  const envelope = EventList.parse(raw);
  const rows = Array.isArray(envelope.data) ? envelope.data : envelope.data.events;
  const events = rows.slice(0, limit);
  const total = Array.isArray(envelope.data) ? envelope.total : envelope.data.total;
  return { events, hasMore: total === undefined ? rows.length > limit : offset + events.length < total };
}

export async function loadModerationEvents(identity: AuthIdentity, signal?: AbortSignal) {
  const limit = 100;
  const catalogue = new Map<string, ManagedEvent>();
  for (let offset = 0; ; offset += limit) {
    // Wallet-signed lists include the pending rows but reserve approval filters for the admin bearer.
    const raw = await signedGetJSON("/events/api/events", {
      identity, signal, query: { list: "active", order: "asc", offset, limit: limit + 1 },
    });
    const { events, hasMore } = eventPage(raw, offset, limit);
    const previousCount = catalogue.size;
    for (const event of events) catalogue.set(event.id, event);
    if (!hasMore || events.length === 0) break;
    if (catalogue.size === previousCount) throw new Error("The moderation catalogue did not advance. Retry to reload the queue.");
  }
  const now = Date.now();
  const cutoff = now - 24 * 60 * 60_000;
  const active = [...catalogue.values()].filter(event => !event.rejected && !event.deleted_by_user && !event.deleted_by_admin);
  return {
    events: active.filter(event => !event.approved && Date.parse(eventOccurrence(event).finish ?? "") > now),
    recentlyApproved: active.filter(event => event.approved && Date.parse(event.updated_at ?? "") >= cutoff),
    hasMore: false,
  };
}

export async function moderateEvent(identity: AuthIdentity, id: string, action: "approve" | "reject", reason?: string): Promise<void> {
  const body = action === "approve" ? { approved: true } : { rejected: true, rejection_reason: reason?.trim().slice(0, 500) || null };
  const result = apiOkOf(z.object({ approved: z.boolean(), rejected: z.boolean() })).parse(await postJSON(`/events/api/events/${encodeURIComponent(id)}`, body, { identity, method: "PATCH" })).data;
  if (action === "approve" ? !result.approved || result.rejected : !result.rejected) {
    throw new Error("The action was not applied. Another reviewer may be required, or the hangout's state may have changed. Refresh the list before trying again.");
  }
}

export async function loadEventProfiles(identity: AuthIdentity, signal?: AbortSignal): Promise<EventProfile[]> {
  return apiOkOf(z.array(Profile)).parse(await signedGetJSON("/events/api/profiles/settings", { identity, signal })).data;
}

/** One wallet's stored grants, a moderator's read; null when the wallet never saved settings. */
export async function loadEventProfileSettings(identity: AuthIdentity, address: string, signal?: AbortSignal): Promise<EventProfile | null> {
  if (!/^0x[0-9a-fA-F]{40}$/.test(address)) throw new Error("Enter a valid wallet address.");
  return apiOkOf(Profile.nullable()).parse(await signedGetJSON(`/events/api/profiles/${address.toLowerCase()}/settings`, { identity, signal })).data;
}

export async function saveEventPermissions(identity: AuthIdentity, address: string, permissions: string[]): Promise<EventProfile> {
  if (!/^0x[0-9a-fA-F]{40}$/.test(address)) throw new Error("Enter a valid wallet address.");
  if (permissions.some((permission) => !EVENT_PERMISSIONS.some((entry) => entry.id === permission))) throw new Error("Choose a supported event permission.");
  return apiOkOf(Profile).parse(await postJSON(`/events/api/profiles/${address.toLowerCase()}/settings`, { permissions }, { identity, method: "PATCH" })).data;
}

/** An account's event permissions and, for a moderator, every account's grants. */
export type EventAccess = { profile: EventProfile; profiles: EventProfile[] | null };

/** The signed reads of the admin pages are the account's own, so they sit under `["private", account, ...]`. */
export const eventAdminKeys = {
  all: (account: string) => ["private", account.toLowerCase(), "event-admin"] as const,
  access: (account: string) => ["private", account.toLowerCase(), "event-admin", "access"] as const,
  queue: (account: string) => ["private", account.toLowerCase(), "event-admin", "queue"] as const,
};

/** The member's own events (My Events), read signed, so private to the account. */
export const ownedEventsKeys = {
  mine: (account: string) => ["private", account.toLowerCase(), "events", "mine"] as const,
};

export type EventReviewStatus = "pending" | "approved" | "rejected";

/** Where an event stands in review; a rejection wins over a stale approval. */
export function eventReviewStatus(event: Pick<ManagedEvent, "approved" | "rejected">): EventReviewStatus {
  if (event.rejected) return "rejected";
  return event.approved ? "approved" : "pending";
}

/**
 * The occurrence an event is shown by, as decentraland.org: a recurring event with a next occurrence
 * counts by it (start and finish together), any other by its own start and finish.
 */
export function eventOccurrence(event: Pick<ManagedEvent, "recurrent" | "start_at" | "finish_at" | "next_start_at" | "next_finish_at">): { start: string | null; finish: string | null } {
  return event.recurrent && event.next_start_at && event.next_finish_at
    ? { start: event.next_start_at, finish: event.next_finish_at }
    : { start: event.start_at, finish: event.finish_at };
}

export function editEventHref(id: string): string {
  return `/events/edit-event/${encodeURIComponent(id)}`;
}

const EVENT_ADMIN_STALE_MS = 30_000;

/**
 * Every account's grants, or null when the moderator-only list refuses this account (403): the server,
 * not the `edit_any_profile` hint, decides who may read other accounts. Any other failure throws.
 */
async function loadEventProfilesIfAllowed(identity: AuthIdentity, signal?: AbortSignal): Promise<EventProfile[] | null> {
  try {
    return await loadEventProfiles(identity, signal);
  } catch (error) {
    if (error instanceof CatalystError && error.status === 403) return null;
    throw error;
  }
}

/** The account's own grants, and every account's when it may administer users (`profiles` null when not). */
export async function loadEventAccess(identity: AuthIdentity, signal?: AbortSignal): Promise<EventAccess> {
  const profile = await loadEventProfile(identity, signal);
  const profiles = profile.permissions.includes("edit_any_profile")
    ? await loadEventProfilesIfAllowed(identity, signal)
    : null;
  return { profile, profiles };
}

export const eventAccessQuery = (identity: AuthIdentity) =>
  queryOptions({
    queryKey: eventAdminKeys.access(identity.signer),
    staleTime: EVENT_ADMIN_STALE_MS,
    queryFn: ({ signal }) => loadEventAccess(identity, signal),
  });

export const moderationQueueQuery = (identity: AuthIdentity) =>
  queryOptions({
    queryKey: eventAdminKeys.queue(identity.signer),
    staleTime: EVENT_ADMIN_STALE_MS,
    queryFn: ({ signal }) => loadModerationEvents(identity, signal),
  });
