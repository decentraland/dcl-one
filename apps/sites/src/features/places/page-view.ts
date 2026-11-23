import type { PageView } from "@/core/lib/telemetry/page-view";
import type { Place } from "@/data/lib/catalyst/schema";

/** The legacy Places page view, under its English title (`discover.home.page_title`): the page has no locale switch. */
export const PLACES_PAGE_VIEW: PageView = { name: "Places — Decentraland" };

/** The legacy Communities tab's page view, under its English title (`discover.communities.page_title`). */
export const COMMUNITIES_PAGE_VIEW: PageView = { name: "Communities — Decentraland" };

/** The legacy scene page view, once the place is read and has a title: its kind, id and world name. */
export function placePageView(place: Pick<Place, "id" | "title" | "world" | "world_name"> | null | undefined): PageView | null {
  if (!place?.title) return null;
  return { name: place.title, properties: { kind: place.world ? "world" : "place", place_id: place.id, world: place.world_name ?? null } };
}
