import { useState, type ReactNode } from "react";

import { CatalogCard, CollectibleBadges } from "@dcl/one-ui/apps/sites/provisional/CatalogCard/CatalogCard";
import { EmptyState } from "@dcl/one-ui/apps/sites/provisional/EmptyState/EmptyState";
import { FilterChip, FilterChipGroup } from "@dcl/one-ui/apps/sites/provisional/FilterChip/FilterChip";
import {
  ProfileCollectibles,
  ProfileCollectiblesItem,
  ProfileNameCard,
  type ProfileCollectiblesState,
} from "@dcl/one-ui/apps/sites/provisional/profile/ProfileCollectibles";
import { Button } from "@dcl/one-ui/apps/sites/shared/Button/Button";
import {
  BackpackUpperBody,
  GeneralGrid,
  GeneralMapPin,
  GeneralName,
  GeneralSecondaryBarEmotes,
} from "@dcl/one-ui/foundations/icons";

import { assetPage } from "@/core/lib/shop/redirects";
import type { AssetCategory, ProfileAsset } from "@/data/lib/catalyst/marketplace/profile-collectibles";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";

import { catalogueTotal, CollectibleLink, loadedRows, loadMore, useFocusNewRows } from "./collectibles";
import { useProfileAssets } from "./hooks";
import { ProfileShell, type ProfileShellTab } from "./ProfileShell";

/** Where the owner's empty Assets sends them: the Shop. */
export const SHOP_HREF = "/shop";
/** NAMEs are managed under Create; the source's Builder `/names/:name` redirects there. */
export const MANAGE_NAMES_HREF = "/create/names";

export type ProfileAssetsScreenProps = {
  profile: ProfileVM;
  own: boolean;
  tabs: ProfileShellTab[];
  onTab: (id: string) => void;
};

const CATEGORY_FILTERS: { value: AssetCategory; label: string; icon: ReactNode }[] = [
  { value: "wearable", label: "Wearables", icon: <BackpackUpperBody /> },
  { value: "emote", label: "Emotes", icon: <GeneralSecondaryBarEmotes /> },
  { value: "ens", label: "Names", icon: <GeneralName /> },
  { value: "parcel", label: "Lands", icon: <GeneralMapPin /> },
  { value: "estate", label: "Estates", icon: <GeneralGrid /> },
];

function countLabel(total: number): string {
  return total === 1 ? "1 asset" : `${total} assets`;
}

function tokenHref(asset: ProfileAsset): string {
  return assetPage(asset.contractAddress, asset.tokenId, "token");
}

/** A NAME's Edit (under Create) and Transfer (its token's transfer page). */
function NameActions({ asset }: { asset: ProfileAsset }) {
  return (
    <>
      <Button href={MANAGE_NAMES_HREF} variant="secondary" size="sm" aria-label={`Edit ${asset.name}`}>
        Edit
      </Button>
      <Button href={`${tokenHref(asset)}/transfer`} size="sm" aria-label={`Transfer ${asset.name}`}>
        Transfer
      </Button>
    </>
  );
}

function AssetCard({ asset }: { asset: ProfileAsset }) {
  const href = tokenHref(asset);
  return (
    <CatalogCard
      title={asset.name}
      imageUrl={asset.image}
      // The catalogue has no image for a NAME.
      placeholder={asset.category === "ens" ? <GeneralName size={64} /> : undefined}
      href={href}
      rarity={asset.rarity}
      price={asset.price ? { amount: asset.price, network: asset.network } : undefined}
      badges={<CollectibleBadges {...asset.traits} />}
      action={
        asset.category === "ens" ? (
          <NameActions asset={asset} />
        ) : (
          <Button href={href} aria-label={`View ${asset.name}`}>
            View
          </Button>
        )
      }
      LinkComponent={CollectibleLink}
    />
  );
}

/** The owner's Assets tab: every token they hold, or one category, a page at a time. */
export function ProfileAssetsScreen({ profile, own, tabs, onTab }: ProfileAssetsScreenProps) {
  const [category, setCategory] = useState<AssetCategory | null>(null);
  const query = useProfileAssets(profile.address, category);
  const rows = query.data ? loadedRows(query.data) : [];
  const focus = useFocusNewRows(rows.length);

  const filters = (
    <FilterChipGroup label="Assets shown">
      <FilterChip label="All" active={category === null} onSelect={() => setCategory(null)} />
      {CATEGORY_FILTERS.map((option) => (
        <FilterChip
          key={option.value}
          icon={option.icon}
          label={option.label}
          active={category === option.value}
          onSelect={() => setCategory(option.value)}
        />
      ))}
    </FilterChipGroup>
  );

  let state: ProfileCollectiblesState;
  if (query.isError && !query.data) {
    state = {
      kind: "error",
      message: "Assets are unavailable right now.",
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
    state = { kind: "loading", label: "Loading assets" };
  } else {
    if (rows.length === 0) {
      state =
        category !== null
          ? { kind: "empty-note", message: countLabel(0) }
          : own
            ? {
                kind: "empty",
                label: "No assets yet",
                children: (
                  <EmptyState
                    icon={<BackpackUpperBody />}
                    title="No assets yet"
                    subtitle="Go to the Shop and start creating your unique look"
                    action={<Button href={SHOP_HREF}>Go to the Shop</Button>}
                  />
                ),
              }
            : { kind: "empty-note", message: "Wearables, names and lands you own will appear here." };
    } else {
      const names = category === "ens";
      state = {
        kind: "list",
        count: countLabel(catalogueTotal(query.data)),
        layout: names ? "rows" : "grid",
        children: rows.map((asset) => (
          <ProfileCollectiblesItem key={asset.id}>
            {names ? (
              <ProfileNameCard name={asset.name} actions={<NameActions asset={asset} />} />
            ) : (
              <AssetCard asset={asset} />
            )}
          </ProfileCollectiblesItem>
        )),
        listRef: focus.listRef,
            more: loadMore(query, "assets", () => focus.expectRowsFrom(rows.length)),
      };
    }
  }

  return (
    <ProfileShell profile={profile} own={own} tabs={tabs} active="assets" onTab={onTab}>
      <ProfileCollectibles filters={filters} state={state} />
    </ProfileShell>
  );
}
