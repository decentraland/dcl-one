import { useInfiniteQuery } from "@tanstack/react-query";
import { useRef } from "react";

import { CommunityMembersList } from "@dcl/one-ui";

import { useAuth } from "@/data/lib/auth/context";
import { shortAddress } from "@/data/lib/catalyst/format/address";
import type { CommunityInfo } from "@/data/lib/catalyst/social/community-detail";
import { membersQuery } from "@/data/lib/community/queries";
import { useLoadMore } from "@/features/social/community/useLoadMore";
import { faceUrl, roleLabel } from "@/features/social/community/view";

import { listMore, listStatus } from "./list-state";

/**
 * A private community's members are read signed, as the member viewing it (only members see them);
 * a public one's as anyone, which is what the server prefetched.
 */
function useMembers(community: CommunityInfo) {
  const { identity } = useAuth();
  return useInfiniteQuery(membersQuery(community.id, community.privacy === "private" ? identity : null));
}

export function CommunityMembers({ community }: { community: CommunityInfo }) {
  const query = useMembers(community);
  const members = query.data?.pages.flatMap((page) => page.members) ?? [];
  const total = query.data?.pages[0]?.total ?? community.membersCount;
  // The list scrolls inside a fixed height, as source, and its next page loads as it nears the end.
  const scroller = useRef<HTMLDivElement>(null);
  const loadMore = () => void query.fetchNextPage();
  const loader = useLoadMore(
    Boolean(query.hasNextPage) && !query.isFetchNextPageError,
    query.isFetchingNextPage,
    loadMore,
    scroller,
  );
  return (
    <CommunityMembersList
      total={total}
      status={listStatus(query)}
      members={members.map((member) => ({
        address: member.memberAddress,
        name: member.name || shortAddress(member.memberAddress),
        // A path on this site renders the same on the server and in the browser.
        face: faceUrl(member.profilePictureUrl),
        role: roleLabel(member.role),
        claimedName: Boolean(member.hasClaimedName),
      }))}
      onRetry={() => void query.refetch()}
      more={listMore(query, loader, loadMore)}
      scrollerRef={scroller}
    />
  );
}
