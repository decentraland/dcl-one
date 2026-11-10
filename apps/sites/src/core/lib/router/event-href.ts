/** The Events section, as the source serves it (decentraland/sites@cfdfa1a5 src/App.tsx:275). */
export const EVENTS_PATH = "/events";

/** One event's detail: the source's `/events?id=<id>` (src/utils/whatsOnUrl.ts, EVENT_ID_PARAM). */
export function eventDetailHref(id: string): string {
  return `${EVENTS_PATH}?${new URLSearchParams({ id }).toString()}`;
}
