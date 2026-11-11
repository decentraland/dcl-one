import type { AuthIdentity } from "../../auth/types";
import { postJSON } from "../client";
import { eventsApiPath } from "../typed";

/** The member's attendance write; the reply is the event's attendee list. Client-safe: no schema modules. */
export function writeAttendance(
  identity: AuthIdentity,
  eventId: string,
  going: boolean,
  signal?: AbortSignal,
): Promise<unknown> {
  const path = eventsApiPath(going ? "post" : "delete", "/api/events/{event_id}/attendees", { event_id: eventId });
  return postJSON<unknown>(path, undefined, { identity, method: going ? "POST" : "DELETE", signal });
}
