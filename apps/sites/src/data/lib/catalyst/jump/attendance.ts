import { queryOptions } from "@tanstack/react-query";
import { z } from "zod";

import type { AuthIdentity } from "../../auth/types";
import { signedGetJSON } from "../client";
import { writeAttendance } from "../places/events";
import { eventsApiPath } from "../typed";

export const JUMP_STALE_MS = 30_000;

/** Whether the member is attending is their own read, so it sits under `["private", account, ...]`. */
export const jumpKeys = {
  attending: (account: string, eventId: string) =>
    ["private", account.toLowerCase(), "jump", "attending", eventId] as const,
};

const AttendingEnvelope = z.object({ data: z.object({ attending: z.boolean() }).passthrough() }).passthrough();

export async function fetchAttending(identity: AuthIdentity, eventId: string, signal?: AbortSignal): Promise<boolean> {
  const raw = await signedGetJSON(eventsApiPath("get", "/api/events/{event_id}", { event_id: eventId }), {
    identity,
    signal,
  });
  return AttendingEnvelope.parse(raw).data.attending;
}

export async function saveAttending(identity: AuthIdentity, eventId: string, attending: boolean): Promise<void> {
  await writeAttendance(identity, eventId, attending);
}

/** The identity signs the request and is never cached; the key names only its account. */
export const attendingQuery = (identity: AuthIdentity, eventId: string) =>
  queryOptions({
    queryKey: jumpKeys.attending(identity.signer, eventId),
    staleTime: JUMP_STALE_MS,
    queryFn: ({ signal }) => fetchAttending(identity, eventId, signal),
  });
