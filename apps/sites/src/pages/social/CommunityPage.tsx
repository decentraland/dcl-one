import { useQuery } from "@tanstack/react-query";

import {
  CommunityDeciding,
  CommunityFrame,
  CommunityInfoCard,
  CommunityLoading,
  CommunityMembershipAction,
  CommunityPrivateNotice,
  CommunitySections,
} from "@dcl/one-ui/apps/sites/social/CommunityDetail";

import { usePageView } from "@/core/lib/telemetry/page-view";
import { useAuth } from "@/data/lib/auth/context";
import { shortAddress } from "@/data/lib/catalyst/format/address";
import type { CommunityInfo } from "@/data/lib/catalyst/social/community-detail";
import { communityQuery } from "@/data/lib/community/queries";
import SitesLayout from "@/features/components/chrome/SitesLayout";
import { useCreatorProfiles } from "@/features/events/useCreatorProfiles";
import { isRouteFatal } from "@/features/social/community/errors";
import { communityPageView } from "@/features/social/community/page-view";
import { useCommunityMembership, type Membership } from "@/features/social/community/useCommunityMembership";
import { canViewContent, faceUrl, membersLabel, thumbnailUrl } from "@/features/social/community/view";

import { CommunityEvents } from "./CommunityEvents";
import { CommunityMembers } from "./CommunityMembers";

const FAILED_COPY = {
  join: "Failed to join community",
  request: "Failed to send your request to join",
  cancel: "Failed to cancel your request",
} as const;

/**
 * `/social/communities/:id`: the community's card (image, name, privacy, members, owner, the member's
 * call to action and description), then its members and upcoming events side by side, or behind
 * tabs on a narrow screen. A private community shows those only to its members.
 */
export function CommunityPage({ id, now }: { id: string; now: number }) {
  const { data: community } = useQuery({
    ...communityQuery(id),
    // Only a first load without data reaches the route's boundary; a failed refetch keeps the page.
    throwOnError: (error, query) => query.state.data === undefined && isRouteFatal(error),
  });
  usePageView(communityPageView(community));
  return (
    <SitesLayout active="play" overlayNav>
      <CommunityFrame>
        {community ? <CommunityBody community={community} now={now} /> : <CommunityLoading />}
      </CommunityFrame>
    </SitesLayout>
  );
}

function CommunityBody({ community: anonymous, now }: { community: CommunityInfo; now: number }) {
  const { identity } = useAuth();
  const membership = useCommunityMembership(anonymous, identity);
  // The member's own read adds their role and the freshest count.
  const community = membership.viewer ?? anonymous;
  const visible = canViewContent(community);
  // A private community waits for the member's own read before deciding they cannot see it.
  const deciding = !visible && community.privacy === "private" && membership.state.kind === "loading";
  return (
    <>
      <CommunityCard community={community} membership={membership} showDescription={visible} />
      {visible ? (
        <CommunitySections
          members={<CommunityMembers community={community} />}
          events={<CommunityEvents communityId={community.id} now={now} />}
        />
      ) : deciding ? (
        <CommunityDeciding />
      ) : (
        <CommunityPrivateNotice />
      )}
    </>
  );
}

function CommunityCard({
  community,
  membership,
  showDescription,
}: {
  community: CommunityInfo;
  membership: Membership;
  showDescription: boolean;
}) {
  const profiles = useCreatorProfiles([[community.ownerAddress]]);
  const owner = profiles[community.ownerAddress.toLowerCase()];
  return (
    <CommunityInfoCard
      name={community.name}
      privacy={community.privacy}
      membersLabel={membersLabel(community.membersCount)}
      thumbnailSrc={thumbnailUrl(community)}
      owner={{
        name: community.ownerName || owner?.name || shortAddress(community.ownerAddress),
        face: faceUrl(owner?.avatarUrl),
      }}
      action={
        <CommunityMembershipAction
          state={membership.state.kind}
          pending={membership.pending}
          onSignIn={membership.signIn}
          onRetry={membership.retry}
          onJoin={membership.join}
          onRequest={membership.requestToJoin}
          onCancel={membership.cancelRequest}
        />
      }
      error={membership.failed ? FAILED_COPY[membership.failed] : undefined}
      description={showDescription ? community.description : null}
    />
  );
}
