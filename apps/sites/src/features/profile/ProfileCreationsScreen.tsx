import { useState } from "react";

import {
  CatalogCard,
  CollectibleBadges,
  type CatalogCardProps,
} from "@dcl/one-ui/apps/sites/provisional/CatalogCard/CatalogCard";
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

import { catalogueTotal, CollectibleLink, loadedRows, loadMore, receivedAny, useFocusNewRows } from "./collectibles";
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
  const payment = item.payment;
  const price: CatalogCardProps["price"] =
    payment.kind === "credits"
      ? { currency: "credits", amount: payment.credits }
      : payment.kind === "mana"
        ? { currency: "mana", amount: payment.mana, network: item.network }
        : undefined;
  return (
    <CatalogCard
      title={item.name}
      imageUrl={item.thumbnail}
      href={href}
      rarity={item.rarity}
      byline={`By ${creator}`}
      price={price}
      // A credit-eligible sale whose quote failed says so; anything else unpriced is not for sale.
      priceFallback={payment.kind === "creditsUnavailable" ? "Credits price unavailable" : "Not for sale"}
      badges={<CollectibleBadges {...item.traits} />}
      action={
        payment.kind !== "none" ? (
          <Button href={href} aria-label={`Buy ${item.name}`}>
            Buy
          </Button>
        ) : (
          <Button href={href} variant="secondary" aria-label={`View in Shop: ${item.name}`}>
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
  const rows = query.data ? loadedRows(query.data) : [];
  const focus = useFocusNewRows(rows.length, query.dataUpdatedAt + query.errorUpdatedAt);
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
  // With no data yet, a Try again refetch puts the query back to pending, so it shows as loading.
  if (query.isError && !query.data) {
    state = {
      kind: "error",
      message: "Creations are unavailable right now.",
      action: (
        <Button
          variant="secondary"
          onClick={() => {
            focus.expectRowsFrom(0);
            void query.refetch();
          }}
        >
          Try again
        </Button>
      ),
    };
  } else if (!query.data) {
    state = { kind: "loading", label: "Loading creations" };
  } else {
    state =
      rows.length === 0 && receivedAny(query.data)
        ? {
            // Every row of the page failed to read: not an empty list, and later pages may still read.
            kind: "error",
            message: "These creations could not be read right now.",
            action: loadMore(query, "creations", () => focus.expectRowsFrom(0)),
          }
        : rows.length === 0
        ? {
            kind: "empty-note",
            message: own ? "Items you create will appear here." : "This account has not created anything yet.",
          }
        : {
            kind: "list",
            // The source shows no count here; it is only announced.
            count: countLabel(rows.length, catalogueTotal(query.data)),
            showCount: false,
            children: rows.map((item) => (
              <ProfileCollectiblesItem key={item.id}>
                <CreationCard item={item} creator={creator} />
              </ProfileCollectiblesItem>
            )),
            listRef: focus.listRef,
            more: loadMore(query, "creations", () => focus.expectRowsFrom(rows.length)),
          };
  }

  return (
    <ProfileShell profile={profile} own={own} tabs={tabs} active="creations" onTab={onTab}>
      <ProfileCollectibles filters={filters} aside={viewAll} state={state} panelRef={focus.panelRef} />
    </ProfileShell>
  );
}
