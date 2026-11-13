import { useLocation, useNavigate, useNavigation } from "react-router";
import { useState } from "react";

import PlaceDetail from "@ui/explorer/pages/PlaceDetail";
import SitesChrome from "@ui/web/frames/SitesChrome";

import UpstreamUnavailable from "@/features/components/UpstreamUnavailable";
import { loadPlace } from "@/data/lib/catalyst/places/index.server";
import { sceneTravelHref } from "@/data/lib/catalyst/places/world-scenes";
import { loadPlaceDetail } from "@/features/explorer/place-detail.server";
import type { AgentMarkdownHandle } from "@/data/lib/agent/markdown";
import { track } from "@/core/lib/telemetry/track";
import { useSceneChat } from "@/features/explorer/scene-chat/useSceneChat";

import type { Route } from "./+types/places_.$id";

export const handle = { agentMarkdown: "placeDetail" } satisfies AgentMarkdownHandle;

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

export default function PlaceDetailRoute({ loaderData }: Route.ComponentProps) {
  const { place, sid, unavailable, travel, livePlayers, scenes, mobile } = loaderData;
  const [sceneId, setSceneId] = useState<string | null>(null);
  const navigate = useNavigate();
  const navigation = useNavigation();
  const location = useLocation();
  // A picked scene of a world is where Jump in lands; until one is picked it is the world's own entry.
  const picked = sceneId === null ? undefined : scenes.find((s) => s.entityId === sceneId);
  const jumpHref = picked && place?.world_name ? sceneTravelHref(place.world_name, picked.base, travel?.playBase) : travel?.jumpHref;
  // The chat joins the scene's room only where the live view renders it: a desktop visitor with players in the scene.
  const shown = picked ?? scenes[0];
  const chat = useSceneChat(
    place && !mobile && (livePlayers ?? 0) > 0
      ? place.world && place.world_name
        ? { location: place.world_name, parcel: shown?.base ?? place.positions?.[0] ?? place.base_position ?? undefined, sceneId: shown?.entityId }
        : place.base_position
          ? { location: place.base_position }
          : null
      : null,
  );
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
      <SitesChrome active="explore" overlayNav>
        <main className="place-detail-route">
          <UpstreamUnavailable
            title="Places are temporarily unavailable"
            message="We couldn't reach the places service. Please try again in a moment."
            backHref="/places"
            backLabel="Back to Places"
          />
        </main>
      </SitesChrome>
    );
  }

  const launchError = travel?.launchError;
  return (
    <SitesChrome active="explore" overlayNav>
      <main className="place-detail-route">
        <PlaceDetail
          key={place?.id}
          place={
            place
              ? {
                  id: place.id,
                  title: place.title ?? "",
                  description: place.description ?? "",
                  image: place.image,
                  coords: place.base_position ?? "",
                  world: place.world ?? false,
                  worldName: place.world_name,
                  contactName: place.contact_name,
                  owner: place.owner,
                }
              : undefined
          }
          notFound={!place}
          livePlayers={livePlayers ?? 0}
          mobile={mobile}
          onClose={() => navigate("/places")}
          onCreator={(address) => navigate(`/profile/${address}`)}
          jumpHref={jumpHref}
          chat={chat}
          onJumpIn={() => {
            if (!place) return;
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
    </SitesChrome>
  );
}
