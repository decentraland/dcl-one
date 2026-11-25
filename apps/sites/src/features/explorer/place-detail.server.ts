import { data as withInit, redirect, type RouterContextProvider } from "react-router";

import { pageLoaderWith } from "@/core/lib/telemetry/page-loader";
import { playUrl } from "@/data/lib/catalyst/bases";
import { CatalystError } from "@/data/lib/catalyst/client";
import type { Place } from "@/data/lib/catalyst/places/index";
import { isMobileRequest, loadLivePlayers } from "@/data/lib/catalyst/places/live-players.server";
import { loadPlaceTravel, loadWorldScenes } from "@/data/lib/catalyst/places/world-scenes";

/**
 * The scene page's data for the place `find` reads: travel, players in it now and, for a world, its scenes.
 * `path` is the page's own path; with `id`, a place the API resolves to another id redirects to that id's page.
 */
export async function loadPlaceDetail(request: Request, context: Readonly<RouterContextProvider>, find: (signal: AbortSignal) => Promise<Place | null>, { path, id }: { path: string; id?: string }) {
  const { sid, wrap, data } = await pageLoaderWith(request, context, { variant: "base" }, async () => {
    try {
      return { place: await find(request.signal), unavailable: false };
    } catch (err) {
      return { place: null, unavailable: !(err instanceof CatalystError && err.status === 404) };
    }
  });
  const { place, unavailable } = data;
  const url = new URL(request.url);

  // A derive-era parcel id (the places API resolves it to the row serving that parcel today)
  // gets one canonical URL, so crawlers and bookmarks stop carrying the retired id forward.
  if (id !== undefined && place && place.id !== id) {
    return redirect(`/places/${encodeURIComponent(place.id)}${url.search}`, 301);
  }

  const refreshRealm = url.searchParams.get("retry_travel") === "1";
  const resolvedTravel = place ? await loadPlaceTravel(place, { signal: request.signal, refreshRealm }) : null;
  if (refreshRealm) {
    url.searchParams.delete("retry_travel");
    url.searchParams.delete("_routes");
    url.searchParams.delete("_data");
    return redirect(`${path}${url.search}`, 303);
  }
  const travel = resolvedTravel ?? (place ? { playBase: playUrl(), launchError: "Travel is temporarily unavailable. Please retry." } : null);
  const livePlayers = place ? await loadLivePlayers(place, { signal: request.signal }) : 0;
  const scenes = place?.world && place.world_name ? await loadWorldScenes(place.world_name, { signal: request.signal }) ?? [] : [];
  const response = wrap(
    { place, sid, unavailable, travel, livePlayers, scenes, mobile: isMobileRequest(request) },
    { status: place ? 200 : unavailable ? 503 : 404 },
  );
  // The layout (live view or the phone's Jump in card) follows the User-Agent: a shared cache keys on it.
  const headers = new Headers(response.init?.headers);
  headers.append("Vary", "User-Agent");
  return withInit(response.data, { ...response.init, headers });
}
