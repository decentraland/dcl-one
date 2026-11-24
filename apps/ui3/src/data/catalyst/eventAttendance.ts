import { parseEventAttendees, type EventAttendee } from "@web-core/catalyst/events";
import { sendSignedJSON } from "./client";

export async function setEventAttendance(
  id: string,
  interested: boolean,
): Promise<EventAttendee[]> {
  const env = await sendSignedJSON<{ data?: unknown[] }>(
    `/api/events/${encodeURIComponent(id)}/attendees`,
    { service: "events", method: interested ? "POST" : "DELETE" },
  );
  return parseEventAttendees(env?.data ?? []);
}
