import { JumpCard, JumpPage } from "@dcl/one-ui/apps/sites";
import { redirect } from "react-router";
import SitesLayout from "@/features/components/chrome/SitesLayout";

import { genericPlaceCard, readJumpTarget, toPlaceCard } from "@/data/lib/catalyst/jump/jump";
import { loadCreator, loadJumpPlace, loadParcelCreator, loadWorldEntity } from "@/data/lib/catalyst/jump/jump.server";
import { detectPlatform } from "@/features/download/options";
import { jumpInFor, mobileLaunch } from "@/features/jump/mobileLaunch";
import { cardProps } from "@/features/jump/view";

import { EVENTS_SECTION, destinationMeta, jumpCanonicalPath, jumpPosition, parcelCard, sectionCard, worldCard } from "../lib/destination-meta";
import { absoluteUrl, publicBase } from "../lib/social-meta";
import type { Route } from "./+types/jump.places._index";

/**
 * The card of the World or parcel the link names, from the catalogue row the loader read; a link naming
 * neither gets the section card, as decentraland.org's did.
 */
export function meta({ loaderData, location, matches }: Route.MetaArgs) {
  const params = new URLSearchParams(location.search);
  const world = readJumpTarget(params).realm;
  const position = jumpPosition(params);
  const row = loaderData?.place ?? null;
  const card = world ? worldCard(world, row) : position ? parcelCard(position, row) : sectionCard(EVENTS_SECTION);
  return destinationMeta(card, { url: absoluteUrl(publicBase(matches), jumpCanonicalPath(location.pathname, params, "place")) });
}

const INVALID = "/jump/places/invalid";

/**
 * A malformed position or a World the worlds server does not know lands on the invalid page. A failed
 * catalogue read is an outage (503), not a bad link; a worlds-server outage still renders the card.
 */
export async function loader({ request }: Route.LoaderArgs) {
  const target = readJumpTarget(new URL(request.url).searchParams);
  if (!target.validPosition || !target.validRealm) throw redirect(INVALID);
  const opts = { signal: request.signal };
  const world = target.realm ?? null;
  const [entity, place, parcelCreator] = await Promise.all([
    // undefined: the worlds server could not answer, which is not "unknown World".
    world ? loadWorldEntity(world, opts).catch(() => undefined) : undefined,
    loadJumpPlace(target, opts).then(
      (record) => ({ record }),
      (error: unknown) => {
        // A visitor who navigated away aborted the read: that is not an outage.
        if (request.signal.aborted) throw error;
        throw new Response("Places are temporarily unavailable", { status: 503 });
      },
    ),
    world ? null : loadParcelCreator(target.coordinates, opts).catch(() => null),
  ]);
  if (entity === null) throw redirect(INVALID);
  const owner = entity?.metadata?.owner;
  const creator = owner ? await loadCreator(owner, opts).catch(() => null) : parcelCreator;
  const card = place.record ? toPlaceCard(place.record) : genericPlaceCard(target.coordinates, target.realm);
  return {
    place: place.record,
    card: cardProps(card, creator),
    launch: mobileLaunch(detectPlatform(request.headers.get("user-agent")), card),
  };
}

/** The launch target depends on the visitor's device. */
export function headers() {
  return { Vary: "User-Agent" };
}

export default function JumpPlaces({ loaderData }: Route.ComponentProps) {
  const { card, launch } = loaderData;
  return (
    <SitesLayout active="play" overlayNav>
      <JumpPage>
        <JumpCard {...card} {...jumpInFor(card.jumpInHref, launch)} />
      </JumpPage>
    </SitesLayout>
  );
}
