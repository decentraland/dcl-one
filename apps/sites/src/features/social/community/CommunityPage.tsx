import { useQuery } from "@tanstack/react-query";
import { useId, useState } from "react";

import SitesChrome from "@ui/web/frames/SitesChrome";

import { Button, Skeleton, Spinner, Tabs, TabsBar } from "@dcl/one-ui";
import { GeneralCheck, GeneralLock, GeneralPlayers, GeneralPrivate } from "@dcl/one-ui/foundations/icons";

import { useAuth } from "@/data/lib/auth/context";
import { shortAddress } from "@/data/lib/catalyst/format/address";
import type { CommunityInfo } from "@/data/lib/catalyst/social/community-detail";
import { communityQuery } from "@/data/lib/community/queries";

import { useCreatorProfiles } from "../../events/useCreatorProfiles";
import { useBrokenImage } from "../../profile/useBrokenImage";
import { CommunityEvents } from "./CommunityEvents";
import { CommunityMembers } from "./CommunityMembers";
import styles from "./CommunityPage.module.css";
import { isRouteFatal } from "./errors";
import { useCommunityMembership, type Membership } from "./useCommunityMembership";
import { canViewContent, faceUrl, membersLabel, thumbnailUrl } from "./view";

type Section = "members" | "events";

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
  return (
    <SitesChrome active="play" overlayNav>
      <div className={styles.root}>
        <div className={styles.content}>
          {community ? <CommunityBody community={community} now={now} /> : <CommunityLoading />}
        </div>
      </div>
    </SitesChrome>
  );
}

function CommunityLoading() {
  return (
    <div className={styles.info} aria-busy="true" aria-label="Loading community">
      <Skeleton tone="neutral" className={styles.thumbnail} />
    </div>
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
        <CommunitySections community={community} now={now} />
      ) : deciding ? (
        <div className={styles.deciding}>
          <Spinner variant="light" label="Loading community" />
        </div>
      ) : (
        <PrivateNotice />
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
  const titleId = useId();
  const profiles = useCreatorProfiles([[community.ownerAddress]]);
  const owner = profiles[community.ownerAddress.toLowerCase()];
  const ownerFace = faceUrl(owner?.avatarUrl);
  const ownerName = community.ownerName || owner?.name || shortAddress(community.ownerAddress);
  return (
    <section className={styles.info} aria-labelledby={titleId}>
      <CommunityThumbnail community={community} />
      <div className={styles.details}>
        <div className={styles.heading}>
          <p className={styles.eyebrow}>Decentraland Community</p>
          <h1 id={titleId} className={styles.title}>
            {community.name}
          </h1>
          <p className={styles.meta}>
            <span className={styles.privacy}>
              {community.privacy === "private" ? (
                <GeneralPrivate size={14} aria-hidden="true" />
              ) : (
                <GeneralPlayers size={14} aria-hidden="true" />
              )}
              {community.privacy === "private" ? "Private" : "Public"}
            </span>
            <span className={styles.divider} aria-hidden="true" />
            <span>{membersLabel(community.membersCount)}</span>
          </p>
          <p className={styles.owner}>
            {ownerFace ? (
              <img className={styles.ownerAvatar} src={ownerFace} alt="" loading="lazy" />
            ) : (
              <span className={styles.ownerAvatar} aria-hidden="true">
                {ownerName.charAt(0).toUpperCase()}
              </span>
            )}
            By <span className={styles.ownerName}>{ownerName}</span>
          </p>
          <div className={styles.actions}>
            <MembershipAction membership={membership} />
          </div>
          {membership.failed ? (
            <p className={styles.error} role="alert">
              {FAILED_COPY[membership.failed]}
            </p>
          ) : null}
        </div>
        {showDescription && community.description ? (
          <p className={styles.description}>{community.description}</p>
        ) : null}
      </div>
    </section>
  );
}

function CommunityThumbnail({ community }: { community: CommunityInfo }) {
  const src = thumbnailUrl(community);
  const { broken, ref, onError } = useBrokenImage(src);
  return (
    <div className={styles.thumbnail}>
      {broken ? (
        <GeneralPlayers className={styles.thumbnailFallback} size={96} aria-hidden="true" />
      ) : (
        <img ref={ref} className={styles.thumbnailImage} src={src} alt={community.name} onError={onError} />
      )}
    </div>
  );
}

function MembershipAction({ membership }: { membership: Membership }) {
  const { state, pending } = membership;
  switch (state.kind) {
    case "joined":
      return (
        <Button variant="outline" size="md" disabled startIcon={<GeneralCheck size={16} aria-hidden="true" />}>
          Joined
        </Button>
      );
    case "signed-out":
      return (
        <Button size="md" onClick={membership.signIn}>
          Sign in to join
        </Button>
      );
    case "loading":
      return (
        <Button variant="secondary" size="md" disabled loading>
          Loading...
        </Button>
      );
    case "error":
      return (
        <>
          <Button variant="secondary" size="md" onClick={membership.retry}>
            Try again
          </Button>
          <p className={styles.error} role="alert">
            We couldn't check your membership right now.
          </p>
        </>
      );
    case "join":
      return (
        <Button size="md" onClick={membership.join} disabled={pending} loading={pending}>
          Join
        </Button>
      );
    case "request":
      return (
        <Button variant="secondary" size="md" onClick={membership.requestToJoin} disabled={pending} loading={pending}>
          Request to join
        </Button>
      );
    case "cancel":
      return (
        <Button variant="secondary" size="md" onClick={membership.cancelRequest} disabled={pending} loading={pending}>
          Cancel request
        </Button>
      );
  }
}

function PrivateNotice() {
  const titleId = useId();
  return (
    <section className={styles.private} aria-labelledby={titleId}>
      <GeneralLock className={styles.privateIcon} size={64} aria-hidden="true" />
      <h2 id={titleId} className={styles.privateTitle}>
        This is a Private Community
      </h2>
      <p className={styles.privateText}>You must be a member of this Community to see its profile.</p>
    </section>
  );
}

function CommunitySections({ community, now }: { community: CommunityInfo; now: number }) {
  const ids = useId();
  const [section, setSection] = useState<Section>("members");
  const tabId = (value: Section) => `${ids}-tab-${value}`;
  const panelId = (value: Section) => `${ids}-panel-${value}`;
  const tab = (value: Section, label: string) => (
    <Tabs
      id={tabId(value)}
      label={label}
      selected={section === value}
      aria-controls={panelId(value)}
      onClick={() => setSection(value)}
    />
  );
  // Side by side from 1024px; below that, the tabs pick which one shows.
  const panelClass = (value: Section) =>
    [styles.panel, styles[value], section === value ? undefined : styles.inactive].filter(Boolean).join(" ");
  return (
    <div className={styles.sections}>
      <TabsBar label="Community sections" inset="content" className={styles.tabs}>
        {tab("members", "Members")}
        {tab("events", "Upcoming Events")}
      </TabsBar>
      <div className={styles.columns}>
        <div id={panelId("members")} role="tabpanel" className={panelClass("members")} aria-labelledby={tabId("members")}>
          <CommunityMembers community={community} />
        </div>
        <div id={panelId("events")} role="tabpanel" className={panelClass("events")} aria-labelledby={tabId("events")}>
          <CommunityEvents communityId={community.id} now={now} />
        </div>
      </div>
    </div>
  );
}
