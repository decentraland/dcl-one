import { useState } from "react";

import {
  CollectiblesItem,
  CollectiblesPanel,
  NameRow,
  useCategoryTabs,
  useFocusNewRows,
  type CategoryOption,
  type CollectiblesState,
  AssetCard,
  NameActions,
  EmptyMessage,
  BackpackUpperBody,
  GeneralGrid,
  GeneralMapPin,
  GeneralName,
  GeneralSecondaryBarEmotes,
  Button,
} from "@dcl/one-ui";


import { assetPage, SHOP_BASE } from "@/core/lib/shop/redirects";
import {
  OPTIONAL_ASSET_CATEGORIES,
  type AssetCategory,
  type ProfileAsset,
} from "@/data/lib/catalyst/marketplace/profile-collectibles";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";

import { catalogueTotal, loadedRows, loadMore, unreadyState, useRouterClick, ShopCardRouterLink } from "./collectibles";
import { useProfileAssetCategories, useProfileAssets } from "./hooks";
import { ProfileShell, type ProfileShellTab } from "./ProfileShell";

/** Where the owner's empty Assets sends them: the Shop. */
export const SHOP_HREF = SHOP_BASE;

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

/** The owner's Assets tab: every token they hold, or one category, a page at a time. */
export function ProfileAssetsScreen({ profile, own, tabs, onTab }: ProfileAssetsScreenProps) {
  const [selectedCategory, setCategory] = useState<AssetCategory | null>(null);
  const optional = useProfileAssetCategories(profile.address);
  const categories = CATEGORY_TABS.filter(({ value }) =>
    !OPTIONAL_ASSET_CATEGORIES.some((category) => category === value) ||
    optional.some((category) => category === value),
  );
  const category = categories.some(({ value }) => value === selectedCategory) ? selectedCategory : null;
  if (category !== selectedCategory) setCategory(category);
  const query = useProfileAssets(profile.address, category);
  const rows = query.data ? loadedRows(query.data) : [];
  const focus = useFocusNewRows(rows.length, query.dataUpdatedAt + query.errorUpdatedAt);
  const routerClick = useRouterClick();

  const { filters, panel } = useCategoryTabs("Assets shown", categories, category, setCategory);

  const names = category === "ens";
  // Assets is only offered on the owner's own profile, so its empty state speaks to the owner.
  const state: CollectiblesState =
    unreadyState(
      query,
      rows,
      {
        noun: "assets",
        unavailable: "Assets are unavailable right now.",
        unreadable: "These assets could not be read right now.",
      },
      focus,
    ) ??
    (!query.data || rows.length === 0
      ? category !== null
        ? { kind: "empty-note", message: countLabel(0) }
        : {
            kind: "empty",
            label: "No assets yet",
            children: (
              <EmptyMessage
                icon={<BackpackUpperBody size={56} />}
                title="No assets yet"
                subtitle="Go to the Shop and start creating your unique look"
                action={
                  <Button href={SHOP_HREF} onClick={routerClick(SHOP_HREF)} size="md">
                    Go to the Shop
                  </Button>
                }
              />
            ),
          }
      : {
          kind: "list",
          count: countLabel(catalogueTotal(query.data)),
          layout: names ? "rows" : "grid",
          children: rows.map((asset) => (
            <CollectiblesItem key={asset.id}>
              {names ? (
                <NameRow
                  name={asset.name}
                  actions={<NameActions name={asset.name} view={tokenHref(asset)} navigate={routerClick} />}
                />
              ) : (
                <AssetCard
                  asset={asset}
                  href={tokenHref(asset)}
                  navigate={routerClick}
                  linkComponent={ShopCardRouterLink}
                />
              )}
            </CollectiblesItem>
          )),
          listRef: focus.listRef,
          more: loadMore(query, "assets", () => focus.expectRowsFrom(rows.length)),
        });

  return (
    <ProfileShell profile={profile} own={own} tabs={tabs} active="assets" onTab={onTab}>
      <CollectiblesPanel filters={filters} panel={panel} state={state} panelRef={focus.panelRef} />
    </ProfileShell>
  );
}
