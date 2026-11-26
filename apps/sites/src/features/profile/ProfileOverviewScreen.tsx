import {
  AvatarBody,
  EquippedItems,
  ProfileOverview,
  CollectiblesItem,
  CollectiblesPanel,
  type CollectiblesState,
  Button,
} from "@dcl/one-ui";

import { playUrl } from "@/data/lib/catalyst/bases";
import type { ProfileBadge } from "@/data/lib/catalyst/social/badges";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";

import { CreationCard } from "./collectibles";
import { useProfileBadges, useWornCollectibles } from "./hooks";
import { ProfileShell, type ProfileShellTab } from "./ProfileShell";

export type ProfileOverviewScreenProps = {
  profile: ProfileVM;
  /** Whether the wallet has a profile at all; badges are read only then. */
  hasProfile: boolean;
  own: boolean;
  tabs: ProfileShellTab[];
  onTab: (id: string) => void;
};

/** The site is in English; a fixed locale keeps the server and browser renders the same. */
const UNLOCKED = new Intl.DateTimeFormat("en-US", { year: "numeric", month: "short", day: "numeric", timeZone: "UTC" });

export function badgeItems(badges: readonly ProfileBadge[]) {
  return badges.map(({ unlockedAt, ...badge }) => ({
    ...badge,
    unlocked: unlockedAt === undefined ? undefined : `Unlocked on ${UNLOCKED.format(unlockedAt)}`,
  }));
}

const NO_WORN_ITEMS = "No equipped items yet.";

/**
 * The collectibles the avatar wears, as the Shop's cards. A visitor gets each card's Buy or View in
 * Shop; on the member's own profile the card only links to the item.
 */
function WornCollectibles({ urns, own }: { urns: readonly string[]; own: boolean }) {
  const query = useWornCollectibles(urns);
  const items = query.data;
  const state: CollectiblesState =
    urns.length === 0
      ? { kind: "empty-note", message: NO_WORN_ITEMS }
      : items === undefined
        ? query.isError
          ? {
              kind: "error",
              message: "Equipped items are unavailable right now.",
              action: (
                <Button variant="secondary" size="md" loading={query.isFetching} onClick={() => void query.refetch()}>
                  Try again
                </Button>
              ),
            }
          : { kind: "loading", label: "Loading equipped items" }
        : items.length === 0
          ? { kind: "empty-note", message: NO_WORN_ITEMS }
          : {
              kind: "list",
              count: items.length === 1 ? "1 equipped item" : `${items.length} equipped items`,
              showCount: false,
              children: items.map((item) => (
                <CollectiblesItem key={item.id}>
                  <CreationCard item={item} actions={!own} />
                </CollectiblesItem>
              )),
            };
  return (
    <EquippedItems>
      <CollectiblesPanel state={state} />
    </EquippedItems>
  );
}

/** Overview with the profile site's look: header, tab bar, avatar column, About, Links and the items worn. */
export function ProfileOverviewScreen({ profile, hasProfile, own, tabs, onTab }: ProfileOverviewScreenProps) {
  const badges = useProfileBadges(profile.address, hasProfile);
  return (
    <ProfileShell
      profile={profile}
      own={own}
      tabs={tabs}
      active="overview"
      onTab={onTab}
      aside={<AvatarBody imageUrl={profile.bodyUrl || undefined} name={profile.name} />}
    >
      <ProfileOverview
        own={own}
        bio={profile.bio.trim()}
        info={profile.info}
        links={profile.links}
        badges={badgeItems(badges.data ?? [])}
        badgesLoading={hasProfile && badges.isPending}
        badgesError={badges.isError && !badges.data}
        editHref={playUrl()}
        equipped={<WornCollectibles urns={profile.wornCollectibles} own={own} />}
      />
    </ProfileShell>
  );
}
