import type { PageView } from "@/core/lib/telemetry/page-view";
import type { CommunityInfo } from "@/data/lib/catalyst/social/community-detail";

/** The legacy community page view, once the community is read. */
export function communityPageView(
  community: Pick<CommunityInfo, "id" | "name" | "privacy" | "membersCount"> | undefined,
): PageView | null {
  if (!community) return null;
  return {
    name: community.name,
    properties: { communityId: community.id, privacy: community.privacy, membersCount: community.membersCount },
  };
}
