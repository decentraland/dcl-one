import { useLocation, useNavigate, useNavigation } from "react-router";
import { useEffect, useState } from "react";

import { PlaceDetail } from "@dcl/one-ui/apps/sites/places/PlaceDetail";
import { isPlaceholderThumbnail, type PlaceViewer } from "@web-core/place-view";
import SitesLayout from "@/features/components/chrome/SitesLayout";

import UpstreamUnavailable from "@/features/components/UpstreamUnavailable";
import { openSignIn } from "@/features/components/auth/signin-store";
import { useAuth } from "@/data/lib/auth/context";
import type { AuthIdentity } from "@/data/lib/auth/types";
import { loadPlace } from "@/data/lib/catalyst/places/index.server";
import { loadPlaceViewer, savePlaceFavorite, savePlaceLike } from "@/data/lib/catalyst/places/interactions";
import { sceneTravelHref } from "@/data/lib/catalyst/places/world-scenes";
import { loadPlaceDetail } from "@/features/explorer/place-detail.server";
import { usePlaceDetailChat, usePlaceDetailCreator } from "@/features/explorer/place-detail-profiles";
import { placePageView } from "@/features/places/page-view";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { track } from "@/core/lib/telemetry/track";
import { useAnalyticsUserId, usePageView } from "@/core/lib/telemetry/page-view";
import { useLegacyTrackScope } from "@/core/lib/telemetry/legacy-click";
import { legacyWatcherHref, trackDiscoverJumpIn, trackDiscoverLaunchScene } from "@/features/places/discover-events";
import { useSceneChat, type SceneChat } from "@/features/explorer/scene-chat/useSceneChat";

import { PLACES_SECTION, placeDetailMeta, sectionCard } from "../lib/destination-meta";
import { publicBase } from "../lib/social-meta";
import type { Route } from "./+types/places_.$id";

export const handle = { agentMarkdown: "placeDetail" } satisfies AgentMarkdownHandle;

/**
 * The place's card, canonical at its own row's page and with its schema.org Place. An unknown place
 * answers 404 and gets no card; while the catalogue is unreachable the page keeps the section card.
 */
export function meta({ loaderData, matches, params }: Route.MetaArgs) {
  return placeDetailMeta(loaderData, publicBase(matches), sectionCard(PLACES_SECTION), `/places/${encodeURIComponent(params.id)}`);
}

export function loader({ request, params, context }: Route.LoaderArgs) {
  const { id } = params;
  return loadPlaceDetail(request, context, (signal) => loadPlace(id, { signal }), { path: `/places/${encodeURIComponent(id)}`, id });
}

export function headers({ loaderHeaders, parentHeaders }: Route.HeadersArgs) {
  const headers = new Headers(parentHeaders);
  const vary = loaderHeaders.get("Vary");
  if (vary) headers.append("Vary", vary);
  return headers;
}

// Likes and favorites are the visitor's own signed writes; signed out, they open sign-in and the button reverts.
function signInFirst(): Promise<never> {
  openSignIn();
  return Promise.reject(new Error("Sign in to rate or save places."));
}

// A world's pane, which never joins a room (see `fetchSceneRoom`).
const WORLD_CHAT: SceneChat = { status: "unavailable", messages: [] };

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
  const { place, sid, unavailable, travel, livePlayers, scenes, mobile } = loaderData;
  const { identity, isConnected } = useAuth();
  const signer = isConnected ? identity : null;
  const viewer = usePlaceViewer(signer, place?.id);
  const [sceneId, setSceneId] = useState<string | null>(null);
  const navigate = useNavigate();
  const navigation = useNavigation();
  const location = useLocation();
  // A picked scene of a world is where Jump in lands; until one is picked it is the world's own entry.
  const picked = sceneId === null ? undefined : scenes.find((s) => s.entityId === sceneId);
  const jumpHref = picked && place?.world_name ? sceneTravelHref(place.world_name, picked.base, travel?.playBase) : travel?.jumpHref;
  // The chat joins the scene's room only where the live view renders it: a desktop visitor with players in the scene.
  // It reads as a guest: the visitor's signer is for the like and favorite writes only. A world place never joins:
  // the node counts each participant of a world's scene rooms against the world's player limit.
  const chat = useSceneChat(place && !place.world && place.base_position && !mobile && (livePlayers ?? 0) > 0 ? { location: place.base_position } : null);
  const creator = usePlaceDetailCreator(place);
  const chatView = usePlaceDetailChat(place?.world ? WORLD_CHAT : chat);
  usePageView(placePageView(place));
  const legacy = { sid, userId: useAnalyticsUserId(), scope: useLegacyTrackScope() };

  function retryTravel() {
    const query = new URLSearchParams(location.search);
    query.set("retry_travel", "1");
    void navigate(`${location.pathname}?${query}`, { replace: true, preventScrollReset: true });
  }

  function onTravel(placeId: string, launchUrl?: string, error?: string) {
    const context = { sid, story: "jump-in" as const };
    track("jump_in_started", { place_id: placeId }, context);
    if (launchUrl) track("jump_in_completed", { place_id: placeId, launch_url: launchUrl }, context);
    else track("jump_in_failed", { place_id: placeId, error: error ?? "Travel is unavailable." }, context);
  }

  if (!place && unavailable) {
    return (
      <SitesLayout active="explore" overlayNav>
        <main className="place-detail-route">
          <UpstreamUnavailable
            title="Places are temporarily unavailable"
            message="We couldn't reach the places service. Please try again in a moment."
            backHref="/places"
            backLabel="Back to Places"
          />
        </main>
      </SitesLayout>
    );
  }

  const launchError = travel?.launchError;
  return (
    <SitesLayout active="explore" overlayNav>
      <main className="place-detail-route">
        <PlaceDetail
          key={place?.id}
          place={
            place
              ? {
                  id: place.id,
                  title: place.title ?? "",
                  description: place.description ?? "",
                  image: isPlaceholderThumbnail(place.image) ? undefined : place.image,
                  coords: place.base_position ?? "",
                  world: place.world ?? false,
                  worldName: place.world_name,
                  approval: place.like_rate == null ? null : Math.round(place.like_rate * 100),
                  favorites: place.favorites ?? 0,
                }
              : undefined
          }
          creator={creator}
          notFound={!place}
          livePlayers={livePlayers ?? 0}
          mobile={mobile}
          onClose={() => navigate("/places")}
          onCreator={(address) => navigate(`/profile/${address}`)}
          jumpHref={jumpHref}
          chat={chatView}
          viewer={viewer}
          account={signer?.signer ?? null}
          onLike={(id, like) => (signer ? savePlaceLike(signer, id, like) : signInFirst())}
          onLaunch={() =>
            trackDiscoverLaunchScene(
              place && travel ? legacyWatcherHref(travel.playBase, place, window.location.origin) : undefined,
              legacy,
            )
          }
          onFavorite={(id, favorite) => (signer ? savePlaceFavorite(signer, id, favorite) : signInFirst())}
          onJumpIn={(from) => {
            if (!place) return;
            trackDiscoverJumpIn(place, from === "modal" ? "jump-in-modal" : "scene-preview", legacy);
            onTravel(place.id, jumpHref, launchError);
            if (!jumpHref) retryTravel();
          }}
          detailSlot={launchError ? <div role="alert"><p>{launchError}</p><button type="button" disabled={navigation.state !== "idle"} onClick={() => { retryTravel(); }}>Retry travel</button></div> : undefined}
          scenePicker={{
            options: scenes.map((s) => ({ value: s.entityId, label: `${s.title} (${s.base})` })),
            value: sceneId ?? scenes[0]?.entityId ?? "",
            onChange: setSceneId,
          }}
        />
      </main>
    </SitesLayout>
  );
}
