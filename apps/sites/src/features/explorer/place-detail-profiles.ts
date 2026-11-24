import { keepPreviousData, useQuery } from "@tanstack/react-query";
import type { PlaceDetailCreator, SceneChatView } from "@dcl/one-ui/apps/sites";
import { getAvatarBackgroundColor, getDisplayName, syntheticAvatar } from "@web-core/profile/avatarColor";

import { FOUNDATION_CONTACT_NAME } from "@/data/lib/catalyst/jump/jump";
import type { Place } from "@/data/lib/catalyst/schema";
import {
  asAddress,
  fetchMemberProfiles,
  MEMBER_PROFILES_STALE_MS,
  sameOriginContentUrl,
} from "@/data/lib/catalyst/social/member-profiles";

import { FOUNDATION_AVATAR_BACKGROUND } from "../places/cards";
import type { SceneChat } from "./scene-chat/useSceneChat";

const profileKeys = {
  creator: (address?: string) => ["place-creator-profile", address] as const,
  sceneChat: (addresses: readonly string[]) => ["scene-chat-profiles", ...addresses] as const,
};

export function usePlaceDetailCreator(place: Pick<Place, "title" | "contact_name" | "owner"> | null): PlaceDetailCreator | undefined {
  const contact = place?.contact_name?.trim();
  const contactName = contact && contact.toLowerCase() !== "sdk" ? contact : undefined;
  const foundation = contactName?.toLowerCase() === FOUNDATION_CONTACT_NAME.toLowerCase();
  const address = contactName && !foundation ? asAddress(place?.owner) ?? undefined : undefined;
  const { data: profiles } = useQuery({
    queryKey: profileKeys.creator(address),
    queryFn: ({ signal }) => fetchMemberProfiles(address ? [address] : [], { signal }),
    enabled: Boolean(address),
    staleTime: MEMBER_PROFILES_STALE_MS,
    retry: false,
  });
  const name = contactName || place?.title?.trim();
  if (!name) return undefined;
  if (foundation) return { name: FOUNDATION_CONTACT_NAME, foundation: true, avatarBackground: FOUNDATION_AVATAR_BACKGROUND };
  const profile = address ? profiles?.[address] : undefined;
  const displayName = address ? getDisplayName(profile?.name || null, profile?.claimed, address) : name;
  const face = profile?.picture ? sameOriginContentUrl(profile.picture) : undefined;
  return { name, address, avatarUrl: face || syntheticAvatar(name), avatarBackground: getAvatarBackgroundColor(displayName) };
}

export function usePlaceDetailChat(chat: SceneChat): SceneChatView {
  const addresses = [...new Set(chat.messages.map((message) => asAddress(message.sender)).filter((address) => address !== null))].sort();
  const { data: profiles } = useQuery({
    queryKey: profileKeys.sceneChat(addresses),
    queryFn: async ({ signal }) => {
      const found = await fetchMemberProfiles(addresses, { signal });
      return Object.fromEntries(addresses.map((address) => [address, found[address] ?? null]));
    },
    enabled: addresses.length > 0,
    staleTime: MEMBER_PROFILES_STALE_MS,
    retry: false,
    // Keep the names already read while a newly seen sender's profile loads.
    placeholderData: keepPreviousData,
  });
  return {
    status: chat.status,
    messages: chat.messages.map((message) => {
      const address = asAddress(message.sender);
      const profile = address ? profiles?.[address] : undefined;
      let name = message.sender || "Anonymous";
      if (address) {
        const fromProfile = getDisplayName(profile?.name || null, profile?.claimed, address);
        // Replace the loading placeholder only once the lookup has answered for this wallet.
        name = fromProfile || (profiles && address in profiles ? `${address.slice(0, 6)}...${address.slice(-4)}` : "…");
      }
      const face = profile?.picture ? sameOriginContentUrl(profile.picture) : undefined;
      return {
        ...message,
        name,
        avatarUrl: face || syntheticAvatar(name === "…" ? address ?? name : name),
        avatarBackground: getAvatarBackgroundColor(name),
      };
    }),
  };
}
