import { useState } from "react";

import type { ShopCardCreator } from "@dcl/one-ui";
import { BackpackUpperBody, GeneralSecondaryBarEmotes } from "@dcl/one-ui/foundations/icons";

import { accountPage } from "@/core/lib/shop/redirects";
import { shortAddress } from "@/data/lib/catalyst/format/address";
import type { CreationsCategory } from "@/data/lib/catalyst/marketplace/profile-collectibles";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";
import { DEFAULT_CREATIONS_CATEGORY } from "@/data/lib/profile/queries";

import {
  catalogueTotal,
  CollectiblesItem,
  CollectiblesPanel,
  CreationCard,
  loadedRows,
  loadMore,
  useCategoryTabs,
  unreadyState,
  useFocusNewRows,
  ViewAllLink,
  type CategoryOption,
  type CollectiblesState,
} from "./collectibles";
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
  return accountPage(address);
}

/** The profile's name, or its short address while it has none. */
function displayName(profile: ProfileVM): string {
  return profile.name && profile.name !== profile.address ? profile.name : shortAddress(profile.address);
}

const CATEGORY_TABS: CategoryOption<CreationsCategory>[] = [
  {
    value: "wearable",
    label: "Wearables",
    icon: <BackpackUpperBody size={20} aria-hidden="true" />,
  },
  {
    value: "emote",
    label: "Emotes",
    icon: <GeneralSecondaryBarEmotes size={20} aria-hidden="true" />,
  },
];

function countLabel(shown: number, total: number): string {
  const noun = (n: number) => (n === 1 ? "1 creation" : `${n} creations`);
  return total > shown ? `Showing ${shown} of ${noun(total)}` : noun(shown);
}

/** The profile's Creations tab: the wearables or emotes a member published, a page at a time. */
export function ProfileCreationsScreen({ profile, own, tabs, onTab }: ProfileCreationsScreenProps) {
  const [category, setCategory] = useState<CreationsCategory>(DEFAULT_CREATIONS_CATEGORY);
  const query = useProfileCreations(profile.address, category);
  const rows = query.data ? loadedRows(query.data) : [];
  const focus = useFocusNewRows(rows.length, query.dataUpdatedAt + query.errorUpdatedAt);
  const creator: ShopCardCreator = {
    name: displayName(profile),
    picture: profile.avatarUrl || undefined,
    color: profile.nameColor,
  };

  const { filters, panel } = useCategoryTabs("Creations shown", CATEGORY_TABS, category, setCategory);

  const viewAll = <ViewAllLink href={creatorShopHref(profile.address)} label="View all" />;

  const state: CollectiblesState =
    unreadyState(
      query,
      rows,
      {
        noun: "creations",
        unavailable: "Creations are unavailable right now.",
        unreadable: "These creations could not be read right now.",
      },
      focus,
    ) ??
    (rows.length === 0 || !query.data
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
            <CollectiblesItem key={item.id}>
              <CreationCard item={item} creator={creator} />
            </CollectiblesItem>
          )),
          listRef: focus.listRef,
          more: loadMore(query, "creations", () => focus.expectRowsFrom(rows.length)),
        });

  return (
    <ProfileShell profile={profile} own={own} tabs={tabs} active="creations" onTab={onTab}>
      <CollectiblesPanel filters={filters} aside={viewAll} panel={panel} state={state} panelRef={focus.panelRef} />
    </ProfileShell>
  );
}
