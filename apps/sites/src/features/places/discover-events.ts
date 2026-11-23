import { sendLegacyDeferred, type LegacySession } from "@/core/lib/telemetry/legacy-click";
import { track } from "@/core/lib/telemetry/track";
import type { DiscoverCategory } from "@/data/lib/catalyst/places/discover";
import type { Place } from "@/data/lib/catalyst/schema";

// The legacy /places (`/discover`) events (decentraland/sites `components/places/*`, `pages/places/DiscoverHomePage`),
// sent through the legacy deferred transport, byte for byte.

type PlacePayload = {
  place_id: string;
  place_title: string | null;
  world: boolean;
  position?: string;
  world_name?: string;
  user_count?: number;
};

/** The legacy `discoverPlacePayload`: an absent field is left out, never null (only the title is passed through). */
export function discoverPlacePayload(place: Pick<Place, "id" | "title" | "world" | "base_position" | "positions" | "world_name" | "user_count">): PlacePayload {
  const position = place.base_position ?? place.positions?.[0];
  return {
    place_id: place.id,
    place_title: place.title,
    world: Boolean(place.world),
    ...(position ? { position } : {}),
    ...(place.world_name ? { world_name: place.world_name } : {}),
    ...(place.user_count != null ? { user_count: place.user_count } : {}),
  };
}

/** Where a Jump In was pressed: the legacy `place` of `Discover Jump In`. */
export type DiscoverJumpInSurface = "live-card" | "place-card" | "jump-in-modal" | "scene-preview";

type DiscoverPlace = Parameters<typeof discoverPlacePayload>[0];

function guarded(run: () => void): void {
  try {
    run();
  } catch {
    // Telemetry never breaks a click.
  }
}

export function trackDiscoverClickLiveCard(place: DiscoverPlace, session: LegacySession): void {
  guarded(() => {
    const props = discoverPlacePayload(place);
    sendLegacyDeferred((audit, ctx) => track("Discover Click Live Card", { ...props, ...audit }, ctx), session);
  });
}

export function trackDiscoverClickPlaceCard(place: DiscoverPlace, session: LegacySession): void {
  guarded(() => {
    const props = discoverPlacePayload(place);
    sendLegacyDeferred((audit, ctx) => track("Discover Click Place Card", { ...props, ...audit }, ctx), session);
  });
}

export function trackDiscoverJumpIn(place: DiscoverPlace, surface: DiscoverJumpInSurface, session: LegacySession): void {
  guarded(() => {
    const props = { ...discoverPlacePayload(place), place: surface };
    sendLegacyDeferred((audit, ctx) => track("Discover Jump In", { ...props, ...audit }, ctx), session);
  });
}

export function trackDiscoverFilterCategory(category: DiscoverCategory, session: LegacySession): void {
  guarded(() => sendLegacyDeferred((audit, ctx) => track("Discover Filter Category", { category, ...audit }, ctx), session));
}

/**
 * The href the legacy `Discover Launch Scene` reported: its Bevy watcher URL (decentraland/sites
 * `DiscoverScenePage.tsx` `buildBevyHref`), built from the same inputs even though this page opens its own
 * travel URL. `playBase` stands for the legacy `BEVY_WEB_URL` (`https://decentraland.org/play/` in production)
 * and is made absolute against `origin`; the location is a world's name or a place's base parcel.
 */
export function legacyWatcherHref(
  playBase: string,
  place: Pick<Place, "world" | "world_name" | "base_position">,
  origin: string,
): string | undefined {
  const location = place.world ? place.world_name : place.base_position;
  if (!location) return undefined;
  const base = new URL(playBase, origin).href;
  const param = location.endsWith(".eth") ? `?realm=${encodeURIComponent(location)}` : `?position=${encodeURIComponent(location)}`;
  return `${base}${param}&systemScene=tortilla.dcl.eth&portables=none&guest=1&hud=0`;
}

/** "Explore the scene" on a place's live view; `href` is `legacyWatcherHref`, left out when there is none. */
export function trackDiscoverLaunchScene(href: string | undefined, session: LegacySession): void {
  guarded(() => {
    const props = href ? { href } : {};
    sendLegacyDeferred((audit, ctx) => track("Discover Launch Scene", { ...props, ...audit }, ctx), session);
  });
}
