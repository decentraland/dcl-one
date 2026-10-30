import { z } from "zod";
import type { AuthIdentity } from "../../auth/types";
import { postJSON, signedGetJSON } from "../client";
import { apiOkOf } from "../envelope";
import { EventRecordSchema } from "../generated-schemas/events";

export const EVENT_PERMISSIONS = [
  { id: "approve_own_event", label: "Approve own hangouts" },
  { id: "approve_any_event", label: "Approve any hangout" },
  { id: "edit_any_event", label: "Edit any hangout" },
  { id: "edit_any_schedule", label: "Manage schedules" },
  { id: "edit_any_profile", label: "Manage user permissions" },
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

export async function loadManagedEvents(identity: AuthIdentity, options: { own: boolean; offset: number; limit: number; search?: string }, signal?: AbortSignal) {
  const raw = await signedGetJSON("/events/api/events", {
    identity, signal,
    query: { list: "all", owner: options.own || undefined, limit: options.limit, offset: options.offset, search: options.search || undefined, order: "desc", ...(!options.own ? { approved: false, rejected: false } : {}) },
  });
  const envelope = EventList.parse(raw);
  const events = Array.isArray(envelope.data) ? envelope.data : envelope.data.events;
  const total = Array.isArray(envelope.data) ? envelope.total : envelope.data.total;
  return { events, hasMore: total === undefined ? events.length === options.limit : options.offset + events.length < total };
}

export async function moderateEvent(identity: AuthIdentity, id: string, action: "approve" | "reject", reason?: string): Promise<void> {
  const body = action === "approve" ? { approved: true } : { rejected: true, rejection_reason: reason?.trim().slice(0, 500) || null };
  await postJSON(`/events/api/events/${encodeURIComponent(id)}`, body, { identity, method: "PATCH" });
}

export async function loadEventProfiles(identity: AuthIdentity, signal?: AbortSignal): Promise<EventProfile[]> {
  return apiOkOf(z.array(Profile)).parse(await signedGetJSON("/events/api/profiles/settings", { identity, signal })).data;
}

export async function saveEventPermissions(identity: AuthIdentity, address: string, permissions: string[]): Promise<EventProfile> {
  if (!/^0x[0-9a-fA-F]{40}$/.test(address)) throw new Error("Enter a valid wallet address.");
  if (permissions.some((permission) => !EVENT_PERMISSIONS.some((entry) => entry.id === permission))) throw new Error("Choose a supported event permission.");
  return apiOkOf(Profile).parse(await postJSON(`/events/api/profiles/${address.toLowerCase()}/settings`, { permissions }, { identity, method: "PATCH" })).data;
}
