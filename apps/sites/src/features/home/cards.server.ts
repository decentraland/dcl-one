import { SITES_LANGUAGES, type WhatsOnCardData } from "@dcl/one-ui";
import {
  getAvatarBackgroundColor,
  getDisplayName,
} from "@web-core/profile/avatarColor";
import type {
  LiveNowCard,
  Profile,
} from "@/data/lib/catalyst/places/live-now.server";
import { homeEventHref } from "./links";
import { formatHomeStarted } from "./locale";

// The reference's cards: a place card
// opens its place page and an event card its event page; the creator's profile names, faces and
// colours the "by" row; Genesis Plaza shows the Decentraland logo on the Foundation's colour.

const FOUNDATION_COLOR = "#9d76e3";

type HomeLiveCard = Omit<WhatsOnCardData, "started"> & {
  started?: { dateTime: string; labels: Record<string, string> };
};

export function whatsOnCards(
  cards: LiveNowCard[],
  profiles: Record<string, Profile>,
  now = Date.now(),
): HomeLiveCard[] {
  return cards.map((card) => {
    const address = card.creatorAddress?.toLowerCase();
    const profile = address ? profiles[address] : undefined;
    const name = profile?.name ?? card.creatorName;
    const color =
      card.isGenesisPlaza && !profile
        ? FOUNDATION_COLOR
        : getAvatarBackgroundColor(
            getDisplayName(name, profile?.hasClaimedName, card.creatorAddress),
          );
    const startedAt = card.startedAt ? Date.parse(card.startedAt) : NaN;
    const minutes = Math.floor((now - startedAt) / 60_000);
    const started = Number.isFinite(minutes) && minutes >= 0
      ? {
          dateTime: new Date(startedAt).toISOString(),
          // The locale can switch without a reload. Serialize each label so hydration uses the server's ICU output.
          labels: Object.fromEntries(
            SITES_LANGUAGES.map(({ code }) => [code, formatHomeStarted(code, minutes)]),
          ),
        }
      : undefined;
    const attendees = card.attendeeAddresses?.flatMap((address) => {
      const person = profiles[address];
      return person?.face
        ? [
            {
              id: address,
              name: getDisplayName(person.name, person.hasClaimedName, address),
              avatarUrl: person.face,
            },
          ]
        : [];
    });
    return {
      id: card.id,
      title: card.title,
      image: card.image || null,
      href: card.place
        ? `/places/place/${card.coordinates}`
        : homeEventHref(card.id),
      users: card.users,
      usersLabel: `${card.users} online`,
      jumpHref: `/play/?position=${encodeURIComponent(card.coordinates)}`,
      started,
      attendees,
      share: !card.place,
      creator: name
        ? {
            name,
            href: profile && address ? `/profile/${address}` : undefined,
            avatarUrl: profile?.face ?? undefined,
            foundation: card.isGenesisPlaza && !profile?.face,
            color,
          }
        : undefined,
    };
  });
}
