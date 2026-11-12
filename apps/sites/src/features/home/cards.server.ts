import type { WhatsOnCardData } from "@dcl/one-ui/apps/sites/home/WhatsOn";
import {
  getAvatarBackgroundColor,
  getDisplayName,
} from "@ui/shop/discovery/lib/avatarColor";
import type {
  LiveNowCard,
  Profile,
} from "@/data/lib/catalyst/places/live-now.server";
import { homeEventHref } from "./links";

// The reference's cards (sites@17b3535a, components/Home/WhatsOn/WhatsOnCard.tsx): a place card
// opens its place page and an event card its event page; the creator's profile names, faces and
// colours the "by" row; Genesis Plaza shows the Decentraland logo on the Foundation's colour.

const FOUNDATION_COLOR = "#9d76e3";

export function whatsOnCards(
  cards: LiveNowCard[],
  profiles: Record<string, Profile>,
): WhatsOnCardData[] {
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
