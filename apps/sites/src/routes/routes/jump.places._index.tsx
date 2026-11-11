import "@dcl/one-ui/foundations/foundations.css";
import { JumpCard } from "@dcl/one-ui/apps/sites/jump/JumpCard";
import { JumpPage } from "@dcl/one-ui/apps/sites/jump/JumpPage";
import { redirect } from "react-router";
import SitesChrome from "@ui/web/frames/SitesChrome";

import { genericPlaceCard, isEnsRealm, readJumpTarget, toPlaceCard } from "@/data/lib/catalyst/jump/jump";
import { loadCreator, loadJumpPlace, loadParcelCreator, loadWorldEntity } from "@/data/lib/catalyst/jump/jump.server";
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
  const [entity, place, parcelCreator] = await Promise.all([
    // undefined: the worlds server could not answer, which is not "unknown World".
    world ? loadWorldEntity(world, opts).catch(() => undefined) : undefined,
    loadJumpPlace(target, opts).then(
      (record) => ({ record }),
      () => null,
    ),
    world ? null : loadParcelCreator(target.coordinates, opts).catch(() => null),
  ]);
  if (entity === null || !place) throw redirect(INVALID);
  const owner = entity?.metadata?.owner;
  const creator = owner ? await loadCreator(owner, opts).catch(() => null) : parcelCreator;
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
