import "@dcl/one-ui/foundations/foundations.css";
import { JumpCard } from "@dcl/one-ui/apps/sites/jump/JumpCard";
import { JumpPage } from "@dcl/one-ui/apps/sites/jump/JumpPage";
import { redirect } from "react-router";
import SitesChrome from "@ui/web/frames/SitesChrome";

import { genericPlaceCard, isEnsRealm, readJumpTarget, toPlaceCard } from "@/data/lib/catalyst/jump/jump";
import { loadJumpPlace, loadSceneCreator, worldExists } from "@/data/lib/catalyst/jump/jump.server";
import { cardProps } from "@/features/jump/view";

import type { Route } from "./+types/jump.places._index";

export function meta() {
  return [{ title: "Jump into Decentraland" }];
}

const INVALID = "/jump/places/invalid";

/**
 * A malformed position, a failed catalogue read or a World the worlds server does not know lands on the
 * invalid page. A worlds-server outage is not "unknown": that card still renders from the catalogue.
 */
export async function loader({ request }: Route.LoaderArgs) {
  const target = readJumpTarget(new URL(request.url).searchParams);
  if (!target.validPosition) throw redirect(INVALID);
  const opts = { signal: request.signal };
  const world = target.realm && isEnsRealm(target.realm) ? target.realm : null;
  const [exists, place, creator] = await Promise.all([
    world ? worldExists(world, opts).catch(() => true) : true,
    loadJumpPlace(target, opts).then(
      (record) => ({ record }),
      () => null,
    ),
    loadSceneCreator(target, opts).catch(() => null),
  ]);
  if (!exists || !place) throw redirect(INVALID);
  const card = place.record ? toPlaceCard(place.record) : genericPlaceCard(target.coordinates, target.realm);
  return { card: cardProps(card, creator) };
}

export default function JumpPlaces({ loaderData }: Route.ComponentProps) {
  return (
    <SitesChrome active="play">
      <JumpPage>
        <JumpCard {...loaderData.card} />
      </JumpPage>
    </SitesChrome>
  );
}
