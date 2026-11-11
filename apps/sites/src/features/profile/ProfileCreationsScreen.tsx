import { useState } from "react";

import { CatalogCard, CollectibleBadges } from "@dcl/one-ui/apps/sites/provisional/CatalogCard/CatalogCard";
import { FilterChip, FilterChipGroup } from "@dcl/one-ui/apps/sites/provisional/FilterChip/FilterChip";
import {
  ProfileCollectibles,
  ProfileCollectiblesItem,
  ProfileCollectiblesViewAll,
  type ProfileCollectiblesState,
} from "@dcl/one-ui/apps/sites/provisional/profile/ProfileCollectibles";
import { Button } from "@dcl/one-ui/apps/sites/shared/Button/Button";
import { BackpackUpperBody, GeneralSecondaryBarEmotes } from "@dcl/one-ui/foundations/icons";

import { assetPage } from "@/core/lib/shop/redirects";
import { shortAddress } from "@/data/lib/catalyst/format/address";
import type { CreationsCategory, ProfileCreation } from "@/data/lib/catalyst/marketplace/profile-collectibles";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";
import { DEFAULT_CREATIONS_CATEGORY } from "@/data/lib/profile/queries";

import { catalogueTotal, CollectibleLink, loadedRows, loadMore } from "./collectibles";
import { useProfileCreations } from "./hooks";
import { ProfileShell, type ProfileShellTab } from "./ProfileShell";

export type ProfileCreationsScreenProps = {
  profile: ProfileVM;
  own: boolean;
  tabs: ProfileShellTab[];
  onTab: (id: string) => void;
};

/** The Shop page listing everything a creator has published, the source's "View all". */
export function creatorShopHref(address: string): string {
  return `/shop/accounts/${encodeURIComponent(address.toLowerCase())}`;
}

/** The profile's name, or its short address while it has none. */
function displayName(profile: ProfileVM): string {
  return profile.name && profile.name !== profile.address ? profile.name : shortAddress(profile.address);
}

function countLabel(shown: number, total: number): string {
  const noun = (n: number) => (n === 1 ? "1 creation" : `${n} creations`);
  return total > shown ? `Showing ${shown} of ${noun(total)}` : noun(shown);
}

function CreationCard({ item, creator }: { item: ProfileCreation; creator: string }) {
  const href = assetPage(item.contractAddress, item.itemId, "item");
  return (
    <CatalogCard
      title={item.name}
      imageUrl={item.thumbnail}
      href={href}
      rarity={item.rarity}
      byline={`By ${creator}`}
      price={item.price ? { amount: item.price, network: item.network } : undefined}
      // A sold-out item with no listing still says why the price row is empty.
      priceFallback="Not for sale"
      badges={<CollectibleBadges {...item.traits} />}
      action={
        item.price ? (
          <Button href={href} aria-label={`Buy ${item.name}`}>
            Buy
          </Button>
        ) : (
          <Button href={href} variant="secondary" aria-label={`View ${item.name} in Shop`}>
            View in Shop
          </Button>
        )
      }
      LinkComponent={CollectibleLink}
    />
  );
}

/** The profile's Creations tab: the wearables or emotes a member published, a page at a time. */
export function ProfileCreationsScreen({ profile, own, tabs, onTab }: ProfileCreationsScreenProps) {
  const [category, setCategory] = useState<CreationsCategory>(DEFAULT_CREATIONS_CATEGORY);
  const query = useProfileCreations(profile.address, category);
  const creator = displayName(profile);

  const filters = (
    <FilterChipGroup label="Creations shown">
      <FilterChip
        icon={<BackpackUpperBody />}
        label="Wearables"
        active={category === "wearable"}
        onSelect={() => setCategory("wearable")}
      />
      <FilterChip
        icon={<GeneralSecondaryBarEmotes />}
        label="Emotes"
        active={category === "emote"}
        onSelect={() => setCategory("emote")}
      />
    </FilterChipGroup>
  );

  const viewAll = (
    <ProfileCollectiblesViewAll href={creatorShopHref(profile.address)} label="View all" LinkComponent={CollectibleLink} />
  );

  let state: ProfileCollectiblesState;
  if (query.isError && !query.data) {
    state = {
      kind: "error",
      message: "Creations are unavailable right now.",
      action: (
        <Button variant="secondary" onClick={() => void query.refetch()}>
          Try again
        </Button>
      ),
    };
  } else if (!query.data) {
    state = { kind: "loading", label: "Loading creations" };
  } else {
    const items = loadedRows(query.data);
    state =
      items.length === 0
        ? {
            kind: "empty-note",
            message: own ? "Items you create will appear here." : "This account has not created anything yet.",
          }
        : {
            kind: "list",
            // The source shows no count here; it is only announced.
            count: countLabel(items.length, catalogueTotal(query.data)),
            showCount: false,
            children: items.map((item) => (
              <ProfileCollectiblesItem key={item.id}>
                <CreationCard item={item} creator={creator} />
              </ProfileCollectiblesItem>
            )),
            more: loadMore(query, "creations"),
          };
  }

  return (
    <ProfileShell profile={profile} own={own} tabs={tabs} active="creations" onTab={onTab}>
      <ProfileCollectibles filters={filters} aside={viewAll} state={state} />
    </ProfileShell>
  );
}
