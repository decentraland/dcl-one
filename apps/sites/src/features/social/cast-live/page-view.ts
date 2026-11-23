import type { PageView } from "@/core/lib/telemetry/page-view";

/** The English titles of the legacy Cast pages (`page.cast.*.page_title`); Cast has no locale switch. */
export const CAST_STREAMER_TITLE = "Streaming on Decentraland Cast";
export const CAST_WATCHER_TITLE = "Watching on Decentraland Cast";

export type CastPageInfo = { placeName: string; location: string; isWorld: boolean };

/**
 * The legacy Cast page view: the base title until the stream's place is known, then
 * `{place} — {title}` with the place's name, location and kind, each sent once.
 */
export function castPageView(title: string, info: CastPageInfo | null): PageView {
  if (!info) return { name: title };
  return {
    name: info.placeName ? `${info.placeName} — ${title}` : title,
    properties: { placeName: info.placeName, location: info.location, isWorld: info.isWorld },
  };
}
