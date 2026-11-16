import { useState } from "react";

import { Button } from "@dcl/one-ui";
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

import {
  catalogueTotal,
  CollectibleBadges,
  CollectibleCard,
  CollectiblesItem,
  CollectiblesPanel,
  loadedRows,
  loadMore,
  NameRow,
  receivedAny,
  useCategoryTabs,
  useFocusNewRows,
  type CategoryOption,
  type CollectiblesState,
} from "./collectibles";
import { useProfileAssets } from "./hooks";
import { EmptyMessage } from "./EmptyMessage";
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

const ICON = { size: 20, "aria-hidden": true } as const;

const CATEGORY_TABS: CategoryOption<AssetCategory | null>[] = [
  { value: null, label: "All" },
  {
    value: "wearable",
    label: "Wearables",
    icon: <BackpackUpperBody {...ICON} />,
  },
  {
    value: "emote",
    label: "Emotes",
    icon: <GeneralSecondaryBarEmotes {...ICON} />,
  },
  { value: "ens", label: "Names", icon: <GeneralName {...ICON} /> },
  { value: "parcel", label: "Lands", icon: <GeneralMapPin {...ICON} /> },
  { value: "estate", label: "Estates", icon: <GeneralGrid {...ICON} /> },
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
    <CollectibleCard
      title={asset.name}
      imageUrl={asset.image}
      // The catalogue has no image for a NAME.
      placeholder={asset.category === "ens" ? <GeneralName size={64} /> : undefined}
      href={href}
      rarity={asset.rarity}
      price={asset.price ? { currency: "mana", amount: asset.price, network: asset.network } : undefined}
      badges={<CollectibleBadges {...asset.traits} />}
      action={
        asset.category === "ens" ? (
          <NameActions asset={asset} />
        ) : (
          <Button href={href} size="sm" aria-label={`View ${asset.name}`}>
            View
          </Button>
        )
      }
    />
  );
}

/** The owner's Assets tab: every token they hold, or one category, a page at a time. */
export function ProfileAssetsScreen({ profile, own, tabs, onTab }: ProfileAssetsScreenProps) {
  const [category, setCategory] = useState<AssetCategory | null>(null);
  const query = useProfileAssets(profile.address, category);
  const rows = query.data ? loadedRows(query.data) : [];
  const focus = useFocusNewRows(rows.length, query.dataUpdatedAt + query.errorUpdatedAt);

  const { filters, panel } = useCategoryTabs("Assets shown", CATEGORY_TABS, category, setCategory);

  let state: CollectiblesState;
  // With no data yet, a Try again refetch puts the query back to pending, so it shows as loading.
  if (query.isError && !query.data) {
    state = {
      kind: "error",
      message: "Assets are unavailable right now.",
      action: (
        <Button
          variant="secondary"
          size="md"
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
    if (rows.length === 0 && receivedAny(query.data)) {
      // Every row of the page failed to read: not an empty list, and later pages may still read.
      state = {
        kind: "error",
        message: "These assets could not be read right now.",
        action: loadMore(query, "assets", () => focus.expectRowsFrom(0)),
      };
    } else if (rows.length === 0) {
      state =
        category !== null
          ? { kind: "empty-note", message: countLabel(0) }
          : own
            ? {
                kind: "empty",
                label: "No assets yet",
                children: (
                  <EmptyMessage
                    icon={<BackpackUpperBody size={56} />}
                    title="No assets yet"
                    subtitle="Go to the Shop and start creating your unique look"
                    action={
                      <Button href={SHOP_HREF} size="md">
                        Go to the Shop
                      </Button>
                    }
                  />
                ),
              }
            : {
                kind: "empty-note",
                message: "Wearables, names and lands you own will appear here.",
              };
    } else {
      const names = category === "ens";
      state = {
        kind: "list",
        count: countLabel(catalogueTotal(query.data)),
        layout: names ? "rows" : "grid",
        children: rows.map((asset) => (
          <CollectiblesItem key={asset.id}>
            {names ? (
              <NameRow name={asset.name} actions={<NameActions asset={asset} />} />
            ) : (
              <AssetCard asset={asset} />
            )}
          </CollectiblesItem>
        )),
        listRef: focus.listRef,
        more: loadMore(query, "assets", () => focus.expectRowsFrom(rows.length)),
      };
    }
  }

  return (
    <ProfileShell profile={profile} own={own} tabs={tabs} active="assets" onTab={onTab}>
      <CollectiblesPanel filters={filters} panel={panel} state={state} panelRef={focus.panelRef} />
    </ProfileShell>
  );
}
