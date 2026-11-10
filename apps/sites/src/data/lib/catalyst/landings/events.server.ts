import { fetchCommunityProfiles } from "@ui/data/hooks/useCommunityProfiles";
import { asAddress, fetchSceneDeployment } from "@ui/data/catalyst/sceneOwner";

import { publicCatalystUrl } from "../bases";
import { catalystBase, type GetOptions } from "../client";
import type { Event } from "../places/events";
import type { Place } from "../schema";
import { eventDetailHref } from "@/core/lib/router/event-href";

// The home "What's on" cards (source Sites features/events/events.discovery.helpers.ts and
// events.discovery.ts). Source reads the events API and realm-provider hot scenes; here the live
// events and the most active places of dcl-one stand in for them. A place card without a known
// creator gets its scene deployer from this node's own content server; every creator is then named
// and pictured by its profile.

const MIN_USERS = 5;
const MAX_CARDS = 3;
export const FOUNDATION_NAME = "Decentraland Foundation";
const CREATOR_TIMEOUT_MS = 5_000;

export type WhatsOnCard = {
  type: "event" | "place";
  id: string;
  title: string;
  users: number;
  image: string | null;
  coordinates: string;
  /** The event or place page. */
  href: string;
  isGenesisPlaza: boolean;
  creatorName?: string;
  creatorAddress?: string;
  /** The creator's profile face, at the node as browsers reach it. */
  creatorAvatarUrl?: string;
};

// Genesis Plaza is the Genesis City scene on parcel 0,0. Only one scene can occupy a parcel, so this
// cannot be claimed by renaming another scene, unlike a title match: the card is credited to the
// Foundation, with its logo.
const GENESIS_PARCEL = "0,0";
const isGenesisPlaza = (place: Place) =>
  !place.world && (place.positions ?? []).includes(GENESIS_PARCEL);

function eventAt(events: Event[], parcels: string[]): Event | undefined {
  const wanted = new Set(parcels);
  return events.find(
    (event) =>
      event.x !== null &&
      event.y !== null &&
      wanted.has(`${event.x},${event.y}`),
  );
}

function placeCard(place: Place): WhatsOnCard {
  const genesis = isGenesisPlaza(place);
  return {
    type: "place",
    id: place.id,
    title: place.title ?? "",
    users: place.user_count ?? 0,
    image: place.image,
    coordinates: place.base_position,
    href: `/places/${encodeURIComponent(place.id)}`,
    isGenesisPlaza: genesis,
    ...(genesis ? { creatorName: FOUNDATION_NAME } : {}),
  };
}

/** At most 3 cards: crowded scenes hosting a live event, then crowded scenes, then a supplied Genesis Plaza. */
export function buildWhatsOnCards(
  liveEvents: Event[],
  places: Place[],
): WhatsOnCard[] {
  const crowded = places.filter(
    (place) => !place.world && (place.user_count ?? 0) >= MIN_USERS,
  );
  const cards: WhatsOnCard[] = [];
  const usedPlaces = new Set<string>();
  const usedEvents = new Set<string>();

  for (const place of crowded) {
    const event = eventAt(liveEvents, place.positions);
    if (!event || usedEvents.has(event.id)) continue;
    cards.push({
      type: "event",
      id: event.id,
      title: event.name ?? place.title ?? "",
      users: place.user_count ?? 0,
      image: event.image,
      coordinates: `${event.x},${event.y}`,
      href: eventDetailHref(event.id),
      isGenesisPlaza: false,
      ...(event.user_name ? { creatorName: event.user_name } : {}),
      ...(asAddress(event.user)
        ? { creatorAddress: asAddress(event.user)! }
        : {}),
    });
    usedPlaces.add(place.id);
    usedEvents.add(event.id);
  }
  cards.sort((a, b) => b.users - a.users);

  const rest = crowded
    .filter((place) => !usedPlaces.has(place.id))
    .sort((a, b) => (b.user_count ?? 0) - (a.user_count ?? 0));
  cards.push(...rest.map(placeCard));

  if (cards.length < MAX_CARDS) {
    const plaza = places.find(isGenesisPlaza);
    if (plaza && !cards.some((card) => card.id === plaza.id))
      cards.push(placeCard(plaza));
  }

  return cards.slice(0, MAX_CARDS);
}

/**
 * Fills in who made each card, in one profile lookup against this node: place cards without a
 * creator get their scene deployer from its content server, and every creator address gets its
 * profile's name (where the card has none) and face. A failed lookup leaves the cards as they were,
 * so the card falls back to the name's initial.
 */
export async function withCreators(
  cards: WhatsOnCard[],
  opts: GetOptions = {},
  deps: {
    deployment?: typeof fetchSceneDeployment;
    profiles?: typeof fetchCommunityProfiles;
    publicBase?: string;
  } = {},
): Promise<WhatsOnCard[]> {
  const pending = cards.filter(
    (card) =>
      card.type === "place" && !card.creatorName && !card.creatorAddress,
  );
  if (!pending.length && !cards.some((card) => card.creatorAddress))
    return cards;
  const base = catalystBase(opts.base);
  const signal = AbortSignal.timeout(CREATOR_TIMEOUT_MS);
  const request = {
    base,
    signal,
    ...(opts.fetchImpl ? { fetchImpl: opts.fetchImpl } : {}),
  };

  const deployers = new Map<string, string>();
  await Promise.all(
    pending.map(async (card) => {
      const deployment = await (deps.deployment ?? fetchSceneDeployment)(
        card.coordinates,
        request,
      ).catch(() => null);
      const address = asAddress(deployment?.deployer);
      if (address) deployers.set(card.id, address);
    }),
  );
  const withAddresses = cards.map((card) => {
    const address = deployers.get(card.id);
    return address && card.type === "place"
      ? { ...card, creatorAddress: address }
      : card;
  });
  const addresses = [
    ...new Set(
      withAddresses.flatMap((card) =>
        card.creatorAddress ? [card.creatorAddress] : [],
      ),
    ),
  ];
  if (!addresses.length) return cards;

  const profiles = await (deps.profiles ?? fetchCommunityProfiles)(
    addresses,
    request,
  ).catch(() => ({}) as Awaited<ReturnType<typeof fetchCommunityProfiles>>);
  // Faces are read from this node, which may be a loopback twin; the page needs the public origin.
  const publicBase = deps.publicBase ?? publicCatalystUrl();
  const visible = (url: string) =>
    url.startsWith(`${base}/`) ? `${publicBase}${url.slice(base.length)}` : url;
  return withAddresses.map((card) => {
    const profile = card.creatorAddress
      ? profiles[card.creatorAddress]
      : undefined;
    if (!profile) return card;
    return {
      ...card,
      ...(!card.creatorName && profile.name
        ? { creatorName: profile.name }
        : {}),
      ...(profile.picture
        ? { creatorAvatarUrl: visible(profile.picture) }
        : {}),
    };
  });
}
