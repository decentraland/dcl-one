/** Open the existing detail route directly; the legacy /events redirect does not select an event. */
export function homeEventHref(id: string): string {
  return `/whats-on/${encodeURIComponent(id)}`;
}
