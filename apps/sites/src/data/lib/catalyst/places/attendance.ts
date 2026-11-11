import { queryOptions } from "@tanstack/react-query";
import { z } from "zod";

import type { AuthIdentity } from "../../auth/types";
import { postJSON, signedGetJSON } from "../client";
import { eventsApiPath } from "../typed";

/** A member's attendance to an event: their read and their write. Client-safe: no schema modules. */

export const ATTENDANCE_STALE_MS = 30_000;

/** Whether the member is attending is their own read, so it sits under `["private", account, ...]`. */
export const attendanceKeys = {
  attending: (account: string, eventId: string) =>
    ["private", account.toLowerCase(), "attendance", eventId] as const,
};

const AttendingEnvelope = z.object({ data: z.object({ attending: z.boolean() }).passthrough() }).passthrough();

async function fetchAttending(identity: AuthIdentity, eventId: string, signal?: AbortSignal): Promise<boolean> {
  const raw = await signedGetJSON(eventsApiPath("get", "/api/events/{event_id}", { event_id: eventId }), {
    identity,
    signal,
  });
  return AttendingEnvelope.parse(raw).data.attending;
}

/** The identity signs the request and is never cached; the key names only its account. */
export const attendingQuery = (identity: AuthIdentity, eventId: string) =>
  queryOptions({
    queryKey: attendanceKeys.attending(identity.signer, eventId),
    staleTime: ATTENDANCE_STALE_MS,
    queryFn: ({ signal }) => fetchAttending(identity, eventId, signal),
  });

/** The reply is the event's attendee list. */
export function writeAttendance(
  identity: AuthIdentity,
  eventId: string,
  going: boolean,
  signal?: AbortSignal,
): Promise<unknown> {
  const path = eventsApiPath(going ? "post" : "delete", "/api/events/{event_id}/attendees", { event_id: eventId });
  return postJSON<unknown>(path, undefined, { identity, method: going ? "POST" : "DELETE", signal });
}
