import type { PlaceCardData } from "@dcl/one-ui/apps/sites/places/cards/types";
import type { JumpInModalPlace } from "@dcl/one-ui/apps/sites/places/JumpInModal";

import { FOUNDATION_CONTACT_NAME } from "@/data/lib/catalyst/jump/jump";
import type { Destination } from "@/data/lib/catalyst/places/destinations";
import { placeCreatorAddress, type PlaceCreatorProfile } from "@/data/lib/catalyst/places/discover";
import { sceneTravelHref } from "@/data/lib/catalyst/places/world-scenes";

import type { DetectedPlatform } from "../download/options";
import { jumpInFor, mobileLaunch } from "../jump/mobileLaunch";

export type CreatorProfiles = Record<string, PlaceCreatorProfile>;

/** The page a card opens. */
export function placeHref(place: Pick<Destination, "id">): string {
  return `/places/${encodeURIComponent(place.id)}`;
}

/** "SDK" is what sdk-commands writes into every scene it scaffolds: a default, not an author. */
function declaredAuthor(place: Destination): string | undefined {
  const name = place.contact_name?.trim();
  return name && name.toLowerCase() !== "sdk" ? name : undefined;
}

/**
 * Who the card credits, as source: the scene's declared author, with the face of the wallet that
 * deployed it; the Foundation gets its logo; a scene with no author credits its own title and links
 * no wallet.
 */
export function placeCreator(place: Destination, profiles: CreatorProfiles = {}): PlaceCardData["creator"] {
  const author = declaredAuthor(place);
  if (author === FOUNDATION_CONTACT_NAME) return { name: author, foundation: true };
  if (!author) {
    const title = place.title?.trim();
    return title ? { name: title } : undefined;
  }
  const address = placeCreatorAddress(place);
  const avatarUrl = address ? profiles[address]?.avatarUrl : undefined;
  return {
    name: author,
    ...(address ? { href: `/profile/${address}` } : {}),
    ...(avatarUrl ? { avatarUrl } : {}),
  };
}

/** The wallets whose faces the cards show, for one batched profile read. */
export function creatorAddresses(places: readonly Destination[]): string[] {
  const out = new Set<string>();
  for (const place of places) {
    const author = declaredAuthor(place);
    const address = author && author !== FOUNDATION_CONTACT_NAME ? placeCreatorAddress(place) : undefined;
    if (address) out.add(address);
  }
  return [...out];
}

/** One address batch per group, each without the addresses an earlier group already asked for; empty batches dropped. */
export function creatorBatches(groups: readonly (readonly Destination[])[]): string[][] {
  const asked = new Set<string>();
  const batches: string[][] = [];
  for (const group of groups) {
    const fresh = creatorAddresses(group).filter((address) => !asked.has(address));
    fresh.forEach((address) => asked.add(address));
    if (fresh.length) batches.push(fresh);
  }
  return batches;
}

function coordinates(place: Destination): [number, number] {
  const match = /^(-?\d+),(-?\d+)$/.exec(place.base_position ?? "");
  return match ? [Number(match[1]), Number(match[2])] : [0, 0];
}

/** Jump In for this visitor, reusing /jump's helpers: the web explorer on desktop, the app on a phone. */
export function placeJumpIn(place: Destination, platform: DetectedPlatform) {
  const realm = place.world ? place.world_name ?? undefined : undefined;
  return jumpInFor(
    sceneTravelHref(realm ?? null, place.base_position),
    mobileLaunch(platform, { coordinates: coordinates(place), ...(realm ? { realm } : {}) }),
  );
}

export function usersLabel(count: number): string {
  return count === 1 ? "1 person in-world" : `${count} people in-world`;
}

export function placeCard(place: Destination, platform: DetectedPlatform, profiles?: CreatorProfiles): PlaceCardData {
  const users = place.user_count ?? 0;
  const creator = placeCreator(place, profiles);
  const coords = place.world ? place.world_name ?? undefined : place.base_position || undefined;
  const { jumpInHref } = placeJumpIn(place, platform);
  return {
    id: place.id,
    title: place.title?.trim() || "Untitled place",
    image: place.image || null,
    href: placeHref(place),
    ...(users > 0 ? { users, usersLabel: usersLabel(users) } : {}),
    ...(creator ? { creator } : {}),
    ...(coords ? { coords } : {}),
    world: place.world,
    featured: place.highlighted,
    live: place.live,
    ...(place.live_event_name ? { liveEventName: place.live_event_name } : {}),
    ...(jumpInHref ? { jumpHref: jumpInHref } : {}),
  };
}

/** The JUMP IN modal's view of a place; `origin` makes the copied link absolute. */
export function modalPlace(place: Destination, card: PlaceCardData, origin: string): JumpInModalPlace {
  return {
    title: card.title,
    image: card.image,
    ...(place.description ? { description: place.description } : {}),
    ...(card.creator ? { creator: card.creator } : {}),
    ...(card.coords ? { coords: card.coords } : {}),
    ...(card.jumpHref ? { jumpHref: card.jumpHref } : {}),
    shareUrl: `${origin}${card.href}`,
  };
}
