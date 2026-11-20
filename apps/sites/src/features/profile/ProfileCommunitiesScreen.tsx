import { useEffect, useRef } from "react";

import { ProfileCommunities } from "@dcl/one-ui/apps/sites/profile/ProfileCommunities";

import { track } from "@/core/lib/telemetry/track";
import { useAuth } from "@/data/lib/auth/index";
import { playUrl } from "@/data/lib/catalyst/bases";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";

import { communityHref, membersLabel, roleLabel, thumbnailUrl } from "../social/community/view";
import { useProfileCommunities } from "./hooks";
import { ProfileShell, type ProfileShellTab } from "./ProfileShell";

export type ProfileCommunitiesScreenProps = {
  profile: ProfileVM;
  own: boolean;
  tabs: ProfileShellTab[];
  onTab: (id: string) => void;
  sid: string;
};

export function ProfileCommunitiesScreen({ profile, own, tabs, onTab, sid }: ProfileCommunitiesScreenProps) {
  const { identity } = useAuth();
  // The signed list is the owner's view only; a visitor's identity is never sent.
  const list = useProfileCommunities(profile.address, own ? identity : null);
  const failed = list.isError && !list.data;
  const communities = list.data?.communities;
  const origin = typeof window === "undefined" ? "" : window.location.origin;

  const reported = useRef(false);
  useEffect(() => {
    if (!failed || reported.current) return;
    reported.current = true;
    track("profile_communities_load_failed", { address: profile.address }, { sid, story: "landings/profile" });
  }, [failed, profile.address, sid]);

  return (
    <ProfileShell profile={profile} own={own} tabs={tabs} active="communities" onTab={onTab}>
      <ProfileCommunities
        own={own}
        failed={failed}
        total={list.data?.total}
        retry={() => void list.refetch()}
        exploreHref={playUrl()}
        communities={communities?.map((community) => {
          const href = communityHref(community.id);
          return {
            id: community.id,
            name: community.name,
            href,
            copyHref: `${origin}${href}`,
            picture: thumbnailUrl(community),
            joined: own && community.role === "member",
            role: community.role === "member" ? undefined : roleLabel(community.role),
            privacy: community.privacy ?? undefined,
            members: community.membersCount === null ? undefined : membersLabel(community.membersCount),
          };
        })}
      />
    </ProfileShell>
  );
}
