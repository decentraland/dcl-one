import { useEffect, useMemo, useState, type AnchorHTMLAttributes, type ReactNode } from "react";
import { Link } from "react-router";

import {
  CommunityAnnouncements,
  CommunityCreate,
  CommunityDirectory,
  CommunityHangouts,
  CommunityLockedChannel,
  CommunityMemberActions,
  CommunityMembershipAction,
  CommunityOverview,
  CommunityThumb,
  CommunityError,
  GeneralHome,
  GeneralPlayers,
  GeneralSearch,
  HubConfirm,
  HubLoading,
  MenusChannel,
  MenusPending,
  NavbarCommunities,
  NavbarNotifications,
  NavbarPlaces,
  NavLabel,
  SocialExplore,
  SocialHome,
  SocialPanel,
  SocialRequests,
} from "@dcl/one-ui";
import { siteUrl } from "@web-core/site";

import { useAuth } from "@/data/lib/auth/context";
import type { HubCommunity, HubMember, HubRequest, HubRequestDecision } from "@/data/lib/catalyst/social/community-hub";
import { openSignIn } from "@/features/components/auth/signin-store";
import SitesLayout, { type SecondaryNavigation } from "@/features/components/chrome/SitesLayout";
import { thumbnailUrl } from "@/features/social/community/view";
import { canManageMember, isMember, manages } from "@/features/social/hub/access";
import {
  useHubCommunity,
  useHubCreate,
  useHubDirectory,
  useHubMemberChange,
  useHubMembership,
  useHubPosts,
  useHubRequests,
} from "@/features/social/hub/useHub";

type Section = "home" | "explore" | "requests";
type Channel = "summary" | "announcements" | "hangouts" | "members" | "requests";

const SECTIONS: Record<Section, { label: string; icon: ReactNode }> = {
  home: { label: "Communities", icon: <NavbarCommunities size={20} aria-hidden="true" /> },
  explore: { label: "Explore communities", icon: <GeneralSearch size={20} aria-hidden="true" /> },
  requests: { label: "Invitations & requests", icon: <NavbarNotifications size={20} aria-hidden="true" /> },
};
const CHANNELS: Record<Channel, { label: string; icon: ReactNode }> = {
  summary: { label: "Overview", icon: <GeneralHome size={20} aria-hidden="true" /> },
  announcements: { label: "Announcements", icon: <MenusChannel size={20} aria-hidden="true" /> },
  hangouts: { label: "Hangouts", icon: <NavbarPlaces size={20} aria-hidden="true" /> },
  members: { label: "Members", icon: <GeneralPlayers size={20} aria-hidden="true" /> },
  requests: { label: "Membership requests", icon: <MenusPending size={20} aria-hidden="true" /> },
};
const LOCKED: readonly Channel[] = ["announcements", "members", "hangouts"];

export const communityHubHref = (id: string, postId?: string) =>
  `/social?community=${encodeURIComponent(id)}${postId ? `&post=${encodeURIComponent(postId)}` : ""}`;

function HubLink({ href, ...props }: AnchorHTMLAttributes<HTMLAnchorElement> & { href: string }) {
  return <Link to={href} {...props} />;
}

type Confirm = { kind: "request" | "leave"; community: HubCommunity };

/** The community with the image its own page shows: the one the API names, else the one the node keeps. */
const withThumbnail = (community: HubCommunity): HubCommunity => ({ ...community, thumbnailUrl: thumbnailUrl(community) });

/** Answers the request the view names by id, as the hub read it. */
function decideBy(
  rows: readonly HubRequest[],
  id: string,
  decision: HubRequestDecision,
  decide: (row: HubRequest, decision: HubRequestDecision) => void,
) {
  const row = rows.find((candidate) => candidate.id === id);
  if (row) decide(row, decision);
}

/**
 * `/social`: the communities hub. The reader's communities and the ones to discover, their
 * invitations and requests, and, with `?community=`, that community's channels; `&post=` opens its
 * announcements on one post. Sections and channels sit in the Sites sidebar, or its menu on a phone.
 */
export function SocialPage({
  communityId,
  postId,
  onSelect,
}: {
  communityId: string | null;
  postId: string | null;
  /** Opens a community (null: back to the sections). */
  onSelect: (id: string | null) => void;
}) {
  const { identity } = useAuth();
  const viewer = identity?.signer.toLowerCase() ?? null;
  const directory = useHubDirectory(identity);
  const opened = useHubCommunity(identity, communityId);
  const requests = useHubRequests(identity, communityId, opened.managing);
  const membership = useHubMembership(identity);
  const create = useHubCreate(identity);
  const [section, setSection] = useState<Section>("home");
  const [channel, setChannel] = useState<Channel>(postId ? "announcements" : "summary");
  const [creating, setCreating] = useState(false);
  const [confirm, setConfirm] = useState<Confirm | null>(null);

  // Another community opens on its overview, or on its announcements when a post link opened it.
  const [shown, setShown] = useState(communityId);
  if (shown !== communityId) {
    setShown(communityId);
    setChannel(postId ? "announcements" : "summary");
    setConfirm(null);
  }

  const communities = useMemo(() => directory.communities.map(withThumbnail), [directory.communities]);
  const found = useMemo(() => directory.discovery.communities.map(withThumbnail), [directory.discovery.communities]);
  const mine = useMemo(() => communities.filter(isMember), [communities]);
  const listed = communityId ? communities.find((row) => row.id === communityId) : undefined;
  const community =
    communityId && !opened.failed ? (opened.community ? withThumbnail(opened.community) : (listed ?? null)) : null;
  const joined = !!community && isMember(community);
  const places = opened.detail?.places ?? [];
  const showHangouts = opened.pending || places.length > 0 || opened.managing;
  const channels = (Object.keys(CHANNELS) as Channel[]).filter((id) =>
    id === "requests" ? opened.managing : id === "hangouts" ? showHangouts : true,
  );
  useEffect(() => {
    if (channel === "hangouts" && !showHangouts) setChannel("summary");
  }, [channel, showHangouts]);
  const requestsCount = requests.pendingOwn.length;

  const goSection = (next: string) => {
    setSection(next as Section);
    onSelect(null);
  };
  const signIn = () => openSignIn();
  const askToCreate = () => (identity ? setCreating(true) : signIn());

  const navigation: SecondaryNavigation[] = [
    {
      title: "Social",
      active: communityId ? null : section,
      onSelect: goSection,
      items: (["home", "explore", ...(requestsCount > 0 ? ["requests" as const] : [])] as Section[]).map((id) => ({
        id,
        label: (
          <NavLabel icon={SECTIONS[id].icon} count={id === "requests" ? requestsCount : undefined}>
            {SECTIONS[id].label}
          </NavLabel>
        ),
      })),
    },
    ...(mine.length
      ? [
          {
            title: "Your communities",
            active: communityId,
            items: mine.map((row) => ({
              id: row.id,
              href: communityHubHref(row.id),
              label: <NavLabel icon={<CommunityThumb community={row} size="nav" />}>{row.name}</NavLabel>,
            })),
          },
        ]
      : []),
    ...(communityId
      ? [
          {
            title: community?.name ?? "Community",
            active: channel,
            onSelect: (id: string) => setChannel(id as Channel),
            items: channels.map((id) => ({
              id,
              label: (
                <NavLabel icon={CHANNELS[id].icon} count={id === "members" && community ? community.membersCount.toLocaleString("en-US") : undefined}>
                  {CHANNELS[id].label}
                </NavLabel>
              ),
            })),
          },
        ]
      : []),
  ];

  const requestRow = communityId
    ? requests.pendingOwn.find((row) => row.communityId === communityId && row.type === "request_to_join")
    : undefined;
  // Until the member's own requests are known, a private community cannot tell Request from Cancel.
  const membershipState = (target: HubCommunity) =>
    !identity
      ? "signed-out"
      : isMember(target)
        ? "joined"
        : requestRow
          ? "cancel"
          : target.privacy === "public"
            ? "join"
            : requests.own.pending
              ? "loading"
              : requests.own.failed
                ? "error"
                : "request";
  // A failed change shows on the community it was for, not on the next one opened.
  const membershipError =
    community && membership.communityId === community.id
      ? membership.error
      : community && requests.own.answerError?.communityId === community.id
        ? requests.own.answerError.text
        : undefined;
  const membershipAction = community ? (
    <CommunityMembershipAction
      state={membershipState(community)}
      pending={membership.pending !== null || requests.own.busy}
      onSignIn={signIn}
      onRetry={requests.own.retry}
      onJoin={() => void membership.change({ community, kind: "join" }).catch(() => {})}
      onRequest={() => {
        membership.reset();
        setConfirm({ kind: "request", community });
      }}
      onCancel={() => requestRow && requests.own.decide(requestRow, "cancelled")}
      onLeave={() => {
        membership.reset();
        setConfirm({ kind: "leave", community });
      }}
    />
  ) : null;

  let heading: string;
  let subheading: string | null = null;
  let icon: ReactNode;
  let body: ReactNode;
  if (communityId) {
    heading = CHANNELS[channel].label;
    subheading = community?.name ?? null;
    icon = CHANNELS[channel].icon;
    body = !community ? (
      opened.pending && !opened.failed ? (
        <HubLoading label="Opening community" />
      ) : (
        <CommunityError reason={opened.failed ? "unavailable" : "not-found"} onRetry={opened.retry} />
      )
    ) : channel === "requests" && opened.managing ? (
      <SocialRequests
        managing
        rows={requests.managed.rows.filter((row) => row.status === "pending")}
        pending={requests.managed.pending}
        error={requests.managed.error}
        busy={requests.managed.busy}
        onRetry={requests.managed.retry}
        onDecide={(row, decision) => decideBy(requests.managed.rows, row.id, decision, requests.managed.decide)}
        onInvite={requests.managed.invite}
        onOpen={onSelect}
      />
    ) : community.privacy === "private" && !joined && LOCKED.includes(channel) ? (
      <CommunityLockedChannel channel={CHANNELS[channel].label} />
    ) : channel === "announcements" ? (
      <Announcements key={`${viewer}:${community.id}`} community={community} postId={postId} />
    ) : channel === "hangouts" ? (
      <CommunityHangouts name={community.name} places={places} pending={opened.pending} />
    ) : channel === "members" ? (
      <Members community={community} members={opened.detail?.members ?? []} pending={opened.pending} viewer={viewer} />
    ) : (
      <CommunityOverview
        community={community}
        joined={joined}
        membershipAction={membershipAction}
        membershipError={confirm ? undefined : membershipError}
        link={`${typeof window === "undefined" ? "" : window.location.origin}${communityHubHref(community.id)}`}
        postsLocked={community.privacy === "private" && !joined}
        latestPost={opened.latestPost}
        latestPending={opened.latestPending}
        showPlaces={showHangouts}
        places={places}
        placesPending={opened.pending}
        onAnnouncements={() => setChannel("announcements")}
        onHangouts={() => setChannel("hangouts")}
      />
    );
  } else {
    heading = SECTIONS[section].label;
    icon = SECTIONS[section].icon;
    body =
      section === "explore" ? (
        <SocialExplore
          mine={mine}
          communities={found}
          query={directory.search}
          pending={directory.discovery.pending}
          failed={directory.discovery.failed}
          onRetry={directory.discovery.retry}
          onLoadMore={directory.discovery.loadMore}
          loadingMore={directory.discovery.loadingMore}
          communityHref={communityHubHref}
          linkComponent={HubLink}
        />
      ) : section === "requests" && identity ? (
        <SocialRequests
          rows={requests.pendingOwn}
          pending={requests.own.pending}
          error={requests.own.error}
          busy={requests.own.busy}
          onRetry={requests.own.retry}
          onDecide={(row, decision) => decideBy(requests.pendingOwn, row.id, decision, requests.own.decide)}
          onOpen={onSelect}
        />
      ) : (
        <SocialHome
          mine={mine}
          communities={communities}
          pending={directory.pending}
          failed={directory.failed}
          onRetry={directory.retry}
          requestsCount={requestsCount}
          onRequests={() => goSection("requests")}
          onExplore={() => goSection("explore")}
          onCreate={askToCreate}
          communityHref={communityHubHref}
          linkComponent={HubLink}
        />
      );
  }

  return (
    <SitesLayout active="play" secondaryNavigation={navigation}>
      <SocialPanel
        label={communityId ? heading : SECTIONS[section].label}
        heading={heading}
        subheading={subheading}
        icon={icon}
        search={
          !communityId && section === "explore"
            ? { label: "Find a community", value: directory.search, onChange: directory.setSearch }
            : undefined
        }
      >
        {body}
      </SocialPanel>
      {confirm ? (
        <HubConfirm
          title={confirm.kind === "leave" ? "Leave community?" : "Request to join?"}
          body={
            confirm.kind === "leave"
              ? `Are you sure you want to leave ${confirm.community.name}?`
              : `Ask the moderators of ${confirm.community.name} to approve your membership.`
          }
          error={membership.error}
          danger={confirm.kind === "leave"}
          busy={membership.pending !== null}
          confirmLabel={
            confirm.kind === "leave"
              ? "Leave community"
              : membership.pending
                ? "Sending…"
                : "Send request"
          }
          cancelLabel={confirm.kind === "leave" ? "Stay" : "Cancel"}
          onConfirm={() =>
            void membership
              .change({ community: confirm.community, kind: confirm.kind === "leave" ? "leave" : "join" })
              .then(() => setConfirm(null), () => {})
          }
          onCancel={() => {
            if (membership.pending === null) setConfirm(null);
          }}
        />
      ) : null}
      {creating ? (
        <CommunityCreate
          create={async (input) => {
            const result = await create(input);
            onSelect(result.id);
            return result;
          }}
          onDone={() => setCreating(false)}
          contentPolicyUrl={siteUrl("/content")}
          nameClaimUrl="/shop/names"
        />
      ) : null}
    </SitesLayout>
  );
}

function Announcements({ community, postId }: { community: HubCommunity; postId: string | null }) {
  const { identity } = useAuth();
  const viewer = identity?.signer.toLowerCase() ?? null;
  const posts = useHubPosts(identity, community.id);
  const managing = manages(community.role);
  return (
    <CommunityAnnouncements
      posts={posts.posts}
      viewer={viewer}
      canPost={!!viewer && managing}
      isOwner={community.role === "owner"}
      canLike={!!viewer && community.role !== "banned"}
      pending={posts.pending}
      error={posts.error}
      onRetry={posts.retry}
      hasMore={posts.hasMore}
      loadingMore={posts.loadingMore}
      onLoadMore={posts.loadMore}
      postHref={(id) => communityHubHref(community.id, id)}
      focusPostId={postId}
      onPublish={posts.publish}
      onEdit={posts.edit}
      onDelete={posts.remove}
      onLike={posts.like}
    />
  );
}

function Members({
  community,
  members,
  pending,
  viewer,
}: {
  community: HubCommunity;
  members: HubMember[];
  pending: boolean;
  viewer: string | null;
}) {
  const { identity } = useAuth();
  const change = useHubMemberChange(identity, community.id);
  return (
    <CommunityDirectory
      name={community.name}
      members={members}
      pending={pending}
      viewer={viewer}
      actions={(row) => {
        const member = members.find((candidate) => candidate.memberAddress === row.memberAddress);
        if (!member || !canManageMember(community.role, viewer, member)) return null;
        return (
          <CommunityMemberActions
            name={member.name || member.memberAddress}
            moderator={member.role === "moderator" || member.role === "mod"}
            canChangeRole={community.role === "owner"}
            onChange={(next) => change(member, next)}
          />
        );
      }}
    />
  );
}
