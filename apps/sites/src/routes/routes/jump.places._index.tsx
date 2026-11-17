import "@dcl/one-ui/foundations/foundations.css";
import { JumpCard } from "@dcl/one-ui/apps/sites/jump/JumpCard";
import { JumpPage } from "@dcl/one-ui/apps/sites/jump/JumpPage";
import { redirect } from "react-router";
import SitesChrome from "@ui/web/frames/SitesChrome";

import { genericPlaceCard, readJumpTarget, toPlaceCard } from "@/data/lib/catalyst/jump/jump";
import { loadCreator, loadJumpPlace, loadParcelCreator, loadWorldEntity } from "@/data/lib/catalyst/jump/jump.server";
import { detectPlatform } from "@/features/download/options";
import { jumpInFor, mobileLaunch } from "@/features/jump/mobileLaunch";
import { cardProps } from "@/features/jump/view";

import type { Route } from "./+types/jump.places._index";

export function meta() {
  return [{ title: "Jump into Decentraland" }];
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
  const card = place.record ? toPlaceCard(place.record, target.lookup) : genericPlaceCard(target.coordinates, target.realm);
  return {
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
    <SitesChrome active="play">
      <JumpPage>
        <JumpCard {...card} {...jumpInFor(card.jumpInHref, launch)} />
      </JumpPage>
    </SitesChrome>
  );
}
