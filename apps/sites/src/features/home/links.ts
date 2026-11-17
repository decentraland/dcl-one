/** Opens the event's detail on the Events page. */
export function homeEventHref(id: string): string {
  return `/events?id=${encodeURIComponent(id)}`;
}
