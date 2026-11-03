import { useEffect, useState } from "react";
import { redirect, useLocation, useNavigate, useNavigation } from "react-router";

import type { PlaceViewer } from "@ui/data/catalyst/places";
import PlaceDetail from "@ui/explorer/pages/PlaceDetail";

import UpstreamUnavailable from "@/features/components/UpstreamUnavailable";
import { openSignIn } from "@/features/components/auth/signin-store";
import { useAuth } from "@/data/lib/auth/context";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { CatalystError } from "@/data/lib/catalyst/client";
import { type Place } from "@/data/lib/catalyst/places/index";
import { loadPlace } from "@/data/lib/catalyst/places/index.server";
import { loadPlaceViewer, savePlaceFavorite, savePlaceLike } from "@/data/lib/catalyst/places/interactions";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { pageLoaderWith } from "@/core/lib/telemetry/page-loader";
import WorldScenes from "@/features/explorer/WorldScenes";
import { playUrl } from "@/data/lib/catalyst/bases";
import { loadPlaceTravel } from "@/data/lib/catalyst/places/world-scenes";
import { track } from "@/core/lib/telemetry/track";

import type { Route } from "./+types/places_.$id";

export const handle = { agentMarkdown: "placeDetail" } satisfies AgentMarkdownHandle;

export async function loader({ request, params }: Route.LoaderArgs) {
  const { id } = params;

  const { sid, wrap, data } = await pageLoaderWith(
    request,
    
    { variant: "base" },
    async () => {
      let place: Place | null = null;
      let unavailable = false;
      try {
        place = await loadPlace(id, { signal: request.signal });
      } catch (err) {
        if (!(err instanceof CatalystError && err.status === 404)) {
          unavailable = true;
        }
      }
      return { place, unavailable };
    },
  );
  const { place, unavailable } = data;

  // A derive-era parcel id (the places API resolves it to the row serving that parcel today)
  // gets one canonical URL, so crawlers and bookmarks stop carrying the retired id forward.
  if (place && place.id !== id) {
    const { search } = new URL(request.url);
    return redirect(`/places/${encodeURIComponent(place.id)}${search}`, 301);
  }

  const url = new URL(request.url);
  const refreshRealm = url.searchParams.get("retry_travel") === "1";
  const resolvedTravel = place ? await loadPlaceTravel(place, { signal: request.signal, refreshRealm }) : null;
  if (refreshRealm) {
    url.searchParams.delete("retry_travel");
    url.searchParams.delete("_routes");
    url.searchParams.delete("_data");
    return redirect(`/places/${encodeURIComponent(id)}${url.search}`, 303);
  }
  const travel = resolvedTravel ?? (place ? { playBase: playUrl(), launchError: "Travel is temporarily unavailable. Please retry." } : null);
  return wrap(
    { id, place, sid, unavailable, travel },
    { status: place ? 200 : unavailable ? 503 : 404 },
  );
}

// Likes and favorites are the visitor's own signed writes; signed out, they open sign-in and the button reverts.
function signInFirst(): Promise<never> {
  openSignIn();
  return Promise.reject(new Error("Sign in to rate or save places."));
}

// The session lives only in the browser, so the visitor's own like and save are read once it is known.
function usePlaceViewer(signer: AuthIdentity | null, placeId: string | undefined): PlaceViewer | null {
  const [read, setRead] = useState<{ signer: AuthIdentity; placeId: string; viewer: PlaceViewer } | null>(null);
  useEffect(() => {
    if (!signer || !placeId) return;
    const controller = new AbortController();
    loadPlaceViewer(signer, placeId, controller.signal).then(
      (viewer) => setRead({ signer, placeId, viewer }),
      () => {},
    );
    return () => controller.abort();
  }, [signer, placeId]);
  return read && read.signer === signer && read.placeId === placeId ? read.viewer : null;
}

export default function PlaceDetailRoute({ loaderData }: Route.ComponentProps) {
  const { place, sid, unavailable, travel } = loaderData;
  const { identity, isConnected } = useAuth();
  const navigate = useNavigate();
  const navigation = useNavigation();
  const location = useLocation();
  function retryTravel() {
    const query = new URLSearchParams(location.search);
    query.set("retry_travel", "1");
    void navigate(`${location.pathname}?${query}`, { replace: true, preventScrollReset: true });
  }
  const signer = isConnected ? identity : null;
  const viewer = usePlaceViewer(signer, place?.id);

  function onTravel(placeId: string, launchUrl?: string, error?: string) {
    const context = { sid, story: "jump-in" as const };
    track("jump_in_started", { place_id: placeId }, context);
    if (launchUrl) track("jump_in_completed", { place_id: placeId, launch_url: launchUrl }, context);
    else track("jump_in_failed", { place_id: placeId, error: error ?? "Travel is unavailable." }, context);
  }

  if (!place && unavailable) {
    return (
      <main className="place-detail-route">
        <UpstreamUnavailable
          title="Places are temporarily unavailable"
          message="We couldn't reach the places service. Please try again in a moment."
          backHref="/places"
          backLabel="Back to Places"
        />
      </main>
    );
  }

  return (
    <main className="place-detail-route">
      <PlaceDetail
        key={`${place?.id}:${signer?.signer ?? "guest"}`}
        place={
          place
            ? {
                id: place.id,
                title: place.title ?? "",
                image: place.image ?? undefined,
                creator: place.contact_name ?? place.owner ?? "Decentraland",
                views: place.user_count ?? null,
                approval: Math.round((place.like_rate ?? 0) * 100),
                description: place.description ?? "",
                coords: place.base_position ?? "",
                categories: place.categories ?? [],
                world: place.world ?? false,
                worldName: place.world_name ?? null,
                parcels: (place.positions ?? []).length,
                favorites: place.favorites ?? 0,
                updated: place.updated_at
                  ? new Date(place.updated_at).toLocaleDateString("en-GB")
                  : "",
                hue:
                  [...(place.id ?? "")].reduce((a, c) => a + c.charCodeAt(0), 0) % 360,
              }
            : undefined
        }
        onClose={() => navigate("/places")}
        jumpHref={travel?.jumpHref}
        onJumpIn={() => {
          if (!place) return;
          onTravel(place.id, travel?.jumpHref, travel?.launchError);
          if (!travel?.jumpHref) retryTravel();
        }}
        detailSlot={<>
          {travel?.launchError && <div role="alert"><p>{travel.launchError}</p><button type="button" disabled={navigation.state !== "idle"} onClick={() => { retryTravel(); }}>Retry travel</button></div>}
          {place?.world && place.world_name && <WorldScenes worldName={place.world_name} selectedId={place.id} playBase={travel?.playBase} onVisit={onTravel} />}
        </>}
        notFound={!place}
        viewer={viewer}
        onLike={(id, like) => (signer ? savePlaceLike(signer, id, like) : signInFirst())}
        onFavorite={(id, favorite) => (signer ? savePlaceFavorite(signer, id, favorite) : signInFirst())}
      />

    </main>
  );
}
