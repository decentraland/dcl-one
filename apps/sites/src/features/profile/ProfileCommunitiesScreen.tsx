import { useEffect, useRef } from "react";

import { Button, JumpInCTA, Spinner, Tag } from "@dcl/one-ui";
import { GeneralCheck, GeneralCopy, GeneralPlayers, NavbarCommunities } from "@dcl/one-ui/foundations/icons";

import { track } from "@/core/lib/telemetry/track";
import { useAuth } from "@/data/lib/auth/index";
import { playUrl } from "@/data/lib/catalyst/bases";
import type { ProfileCommunity } from "@/data/lib/catalyst/social/profile-communities";
import type { ProfileVM } from "@/data/lib/catalyst/social/profile";

import { useCopyStatus } from "../components/useCopyStatus";
import { communityHref, membersLabel, roleLabel, thumbnailUrl } from "../social/community/view";
import { EmptyMessage } from "./EmptyMessage";
import { useProfileCommunities } from "./hooks";
import styles from "./ProfileCommunitiesScreen.module.css";
import { ProfileShell, type ProfileShellTab } from "./ProfileShell";
import { useBrokenImage } from "./useBrokenImage";

const OWNER_EMPTY_TITLE = "No communities yet";
const MEMBER_EMPTY = "This member has not joined any communities yet.";

export type ProfileCommunitiesScreenProps = {
  profile: ProfileVM;
  own: boolean;
  tabs: ProfileShellTab[];
  onTab: (id: string) => void;
  sid: string;
};

function countLabel(shown: number, total: number): string {
  const noun = (n: number) => (n === 1 ? "1 community" : `${n} communities`);
  return total > shown ? `Showing ${shown} of ${noun(total)}` : noun(shown);
}

function CommunityCard({ community, own }: { community: ProfileCommunity; own: boolean }) {
  const picture = thumbnailUrl(community);
  const href = communityHref(community.id);
  const { broken, ref, onError } = useBrokenImage(picture);
  const { state, announcement, copy } = useCopyStatus();
  const joined = own && community.role === "member";
  return (
    <li className={styles.card}>
      <div className={styles.media}>
        {broken ? (
          <NavbarCommunities size={48} aria-hidden="true" />
        ) : (
          <img ref={ref} className={styles.picture} src={picture} alt="" loading="lazy" decoding="async" onError={onError} />
        )}
        {community.role !== "member" ? (
          <div className={styles.role}><Tag variant="role" className={styles.roleTag}>{roleLabel(community.role)}</Tag></div>
        ) : null}
        {community.membersCount !== null ? (
          <span className={styles.members}>
            <GeneralPlayers size={16} aria-hidden="true" />
            {membersLabel(community.membersCount)}
          </span>
        ) : null}
      </div>
      <div className={styles.body}>
        <h3 className={styles.name}>{community.name}</h3>
        {community.privacy ? (
          <span className={styles.visuallyHidden}>{community.privacy === "private" ? "Private" : "Public"}</span>
        ) : null}
        <div className={styles.actions}>
          <Button
            variant="outline"
            size="sm"
            href={href}
            aria-label={`${joined ? "Joined" : "View"} ${community.name}`}
            leadingIcon={joined ? <GeneralCheck size={16} aria-hidden="true" /> : undefined}
          >
            {joined ? "Joined" : "View"}
          </Button>
          <Button
            variant="outline"
            size="sm"
            aria-label={`Copy link to ${community.name}`}
            onClick={() => copy(new URL(href, window.location.origin).toString())}
          >
            {state === "copied" ? <GeneralCheck size={16} aria-hidden="true" /> : <GeneralCopy size={16} aria-hidden="true" />}
          </Button>
        </div>
        <span className={styles.visuallyHidden} role="status">
          <span key={announcement}>{state === "copied" ? "Community link copied." : state === "failed" ? "Couldn't copy the community link." : ""}</span>
        </span>
      </div>
    </li>
  );
}

export function ProfileCommunitiesScreen({ profile, own, tabs, onTab, sid }: ProfileCommunitiesScreenProps) {
  const { identity } = useAuth();
  // The signed list is the owner's view only; a visitor's identity is never sent.
  const list = useProfileCommunities(profile.address, own ? identity : null);
  const failed = list.isError && !list.data;
  const communities = list.data?.communities;

  const reported = useRef(false);
  useEffect(() => {
    if (!failed || reported.current) return;
    reported.current = true;
    track("profile_communities_load_failed", { address: profile.address }, { sid, story: "landings/profile" });
  }, [failed, profile.address, sid]);

  const empty = !failed && communities?.length === 0;

  return (
    <ProfileShell profile={profile} own={own} tabs={tabs} active="communities" onTab={onTab}>
      <div className={styles.root}>
        {/* One live region, always mounted, so a screen reader hears how loading ended. */}
        <p className={communities?.length ? styles.count : styles.status} role="status">
          {failed ? (
            "Communities are unavailable right now."
          ) : !communities ? (
            <>
              <Spinner variant="light" className={styles.spinner} aria-hidden="true" />
              <span className={styles.visuallyHidden}>Loading communities</span>
            </>
          ) : communities.length > 0 ? (
            countLabel(communities.length, list.data?.total ?? communities.length)
          ) : own ? (
            // The empty message below shows this title; the region only announces it.
            <span className={styles.visuallyHidden}>{OWNER_EMPTY_TITLE}</span>
          ) : (
            <span className={styles.note}>{MEMBER_EMPTY}</span>
          )}
        </p>
        {failed ? (
          <div className={styles.errorAction}>
            <Button variant="secondary" size="md" onClick={() => void list.refetch()}>
              Try again
            </Button>
          </div>
        ) : null}
        {empty && own ? (
          <EmptyMessage
            icon={<NavbarCommunities size={56} />}
            title={OWNER_EMPTY_TITLE}
            subtitle="Jump into Decentraland and join the community that better aligns with you. You can also create your own!"
            action={<JumpInCTA href={playUrl()} size="md" label="Explore communities" />}
          />
        ) : null}
        {communities?.length ? (
          <ul className={styles.grid}>
            {communities.map((community) => (
              <CommunityCard key={community.id} community={community} own={own} />
            ))}
          </ul>
        ) : null}
      </div>
    </ProfileShell>
  );
}
